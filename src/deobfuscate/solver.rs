//! YouTube player challenge solver (Rust port of yt-dlp/ejs).
//!
//! Uses [oxc_parser](https://docs.rs/oxc_parser) for AST analysis and rquickjs to
//! execute the generated solver code. Based on [yt-dlp/ejs](https://github.com/yt-dlp/ejs) (Unlicense).

use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, AssignmentOperator, Expression, FunctionBody, Program, Statement};
use oxc_parser::{ParseOptions, Parser};
use oxc_span::{GetSpan, SourceType, Span};
use rquickjs::{Context, Runtime};

use crate::error::internal::DeobfError;

const SETUP: &str = r#"
if (typeof globalThis.XMLHttpRequest === "undefined") {
    globalThis.XMLHttpRequest = { prototype: {} };
}
if (typeof URL === "undefined") {
    globalThis.location = {
        hash: "",
        host: "www.youtube.com",
        hostname: "www.youtube.com",
        href: "https://www.youtube.com/watch?v=yt-dlp-wins",
        origin: "https://www.youtube.com",
        password: "",
        pathname: "/watch",
        port: "",
        protocol: "https:",
        search: "?v=yt-dlp-wins",
        username: "",
    };
} else {
    globalThis.location = new URL("https://www.youtube.com/watch?v=yt-dlp-wins");
}
if (typeof globalThis.document === "undefined") {
    globalThis.document = Object.create(null);
}
if (typeof globalThis.navigator === "undefined") {
    globalThis.navigator = Object.create(null);
}
if (typeof globalThis.self === "undefined") {
    globalThis.self = globalThis;
}
if (typeof globalThis.window === "undefined") {
    globalThis.window = globalThis;
}
"#;

const INIT: &str = "var _result={n:null,sig:null};";
const WRAPPERS: &str =
    "var deobf_sig=function(a){return _result.sig(a);};var deobf_nsig=function(a){return _result.n(a);};";

/// Extract sig/nsig deobfuscation code from a player script.
pub fn extract_fns(player_js: &str) -> Result<(String, String), DeobfError> {
    let preprocessed = preprocess_player(player_js)?;
    verify_fns(&preprocessed)?;

    Ok((
        format!("{INIT}{preprocessed}"),
        WRAPPERS.to_owned(),
    ))
}

fn preprocess_player(player_js: &str) -> Result<String, DeobfError> {
    let allocator = Allocator::default();
    let source_type = SourceType::default().with_module(false);
    let ret = Parser::new(&allocator, player_js, source_type)
        .with_options(ParseOptions {
            parse_regular_expression: true,
            ..ParseOptions::default()
        })
        .parse();

    if ret.panicked {
        return Err(DeobfError::Extraction("player js parse panic"));
    }
    if !ret.errors.is_empty() {
        return Err(DeobfError::Extraction("player js parse errors"));
    }

    let (prefix_end, suffix_start, statements) = modify_player(&ret.program, player_js)?;
    let solvers = find_solvers(&statements, player_js)?;

    if solvers.is_empty() {
        return Err(DeobfError::Extraction("sig fn name"));
    }

    let mut out = String::with_capacity(player_js.len() + 4096);
    out.push_str(SETUP);
    out.push_str(&player_js[..prefix_end]);
    for stmt in &statements {
        out.push_str(source_slice(player_js, stmt.span()));
        if !source_slice(player_js, stmt.span()).ends_with(';') {
            out.push(';');
        }
    }

    let sig_solvers: Vec<_> = solvers.iter().map(|s| make_solver_fn(s, "sig")).collect();
    let nsig_solvers: Vec<_> = solvers.iter().map(|s| make_solver_fn(s, "n")).collect();
    out.push_str("_result.sig=");
    out.push_str(&multi_try(&sig_solvers));
    out.push(';');
    out.push_str("_result.n=");
    out.push_str(&multi_try(&nsig_solvers));
    out.push(';');
    out.push_str(&player_js[suffix_start..]);

    Ok(out)
}

fn modify_player<'a>(
    program: &'a Program<'a>,
    _source: &str,
) -> Result<(usize, usize, Vec<&'a Statement<'a>>), DeobfError> {
    let body = &program.body;
    let (func_body, skip_first) = match body.len() {
        1 => {
            let Statement::ExpressionStatement(expr_stmt) = &body[0] else {
                return Err(DeobfError::Extraction("unexpected player structure"));
            };
            let Expression::CallExpression(call) = &expr_stmt.expression else {
                return Err(DeobfError::Extraction("unexpected player structure"));
            };
            (iife_body(call)?, false)
        }
        2 => {
            let Statement::ExpressionStatement(expr_stmt) = &body[1] else {
                return Err(DeobfError::Extraction("unexpected player structure"));
            };
            let Expression::CallExpression(call) = &expr_stmt.expression else {
                return Err(DeobfError::Extraction("unexpected player structure"));
            };
            let func_body = direct_iife_body(call)?;
            if func_body.statements.is_empty() {
                return Err(DeobfError::Extraction("unexpected player structure"));
            }
            (func_body, true)
        }
        _ => return Err(DeobfError::Extraction("unexpected player structure")),
    };

    let block_body = if skip_first {
        &func_body.statements[1..]
    } else {
        func_body.statements.as_slice()
    };

    let prefix_end = if skip_first {
        func_body.statements[1].span().start as usize
    } else {
        func_body.statements.first().map(|s| s.span().start as usize).unwrap_or(func_body.span.start as usize)
    };
    let suffix_start = func_body.span.end as usize - 1;

    let mut filtered = Vec::new();
    for stmt in block_body {
        let keep = match stmt {
            Statement::ExpressionStatement(expr_stmt) => matches!(
                &expr_stmt.expression,
                Expression::AssignmentExpression(_)
                    | Expression::BooleanLiteral(_)
                    | Expression::NullLiteral(_)
                    | Expression::NumericLiteral(_)
                    | Expression::StringLiteral(_)
            ),
            _ => true,
        };
        if keep {
            filtered.push(stmt);
        }
    }

    Ok((prefix_end, suffix_start, filtered))
}

fn iife_body<'a>(
    call: &'a oxc_ast::ast::CallExpression<'a>,
) -> Result<&'a FunctionBody<'a>, DeobfError> {
    let Expression::StaticMemberExpression(member) = &call.callee else {
        return Err(DeobfError::Extraction("unexpected player structure"));
    };
    let Some(func) = function_expression(&member.object) else {
        return Err(DeobfError::Extraction("unexpected player structure"));
    };
    func.body
        .as_deref()
        .ok_or(DeobfError::Extraction("unexpected player structure"))
}

fn direct_iife_body<'a>(
    call: &'a oxc_ast::ast::CallExpression<'a>,
) -> Result<&'a FunctionBody<'a>, DeobfError> {
    let Some(func) = function_expression(&call.callee) else {
        return Err(DeobfError::Extraction("unexpected player structure"));
    };
    func.body
        .as_deref()
        .ok_or(DeobfError::Extraction("unexpected player structure"))
}

fn function_expression<'a>(expr: &'a Expression<'a>) -> Option<&'a oxc_ast::ast::Function<'a>> {
    match expr {
        Expression::FunctionExpression(func) => Some(func),
        Expression::ParenthesizedExpression(paren) => function_expression(&paren.expression),
        _ => None,
    }
}

struct SolverCandidate {
    name: String,
}

fn find_solvers(statements: &[&Statement], source: &str) -> Result<Vec<SolverCandidate>, DeobfError> {
    let mut solvers = Vec::new();
    for stmt in statements {
        if let Some(name) = extract_solver_name(stmt, source) {
            solvers.push(SolverCandidate { name });
        }
    }
    Ok(solvers)
}

fn extract_solver_name(stmt: &Statement, source: &str) -> Option<String> {
    for (name_span, body) in function_candidates(stmt) {
        if body_contains_alr_yes(body) {
            return Some(source_slice(source, name_span).to_owned());
        }
    }
    None
}

fn function_candidates<'a>(stmt: &'a Statement<'a>) -> Vec<(Span, &'a FunctionBody<'a>)> {
    match stmt {
        Statement::FunctionDeclaration(func) => {
            if func.r#async || func.generator {
                return vec![];
            }
            let Some(id) = &func.id else {
                return vec![];
            };
            func.body
                .as_deref()
                .map(|body| vec![(id.span, body)])
                .unwrap_or_default()
        }
        Statement::ExpressionStatement(expr_stmt) => {
            let Expression::AssignmentExpression(assign) = &expr_stmt.expression else {
                return vec![];
            };
            if !matches!(assign.operator, AssignmentOperator::Assign) {
                return vec![];
            }
            let Expression::FunctionExpression(func) = &assign.right else {
                return vec![];
            };
            if func.r#async {
                return vec![];
            }
            func.body
                .as_deref()
                .map(|body| vec![(assign.left.span(), body)])
                .unwrap_or_default()
        }
        Statement::VariableDeclaration(var_decl) => var_decl
            .declarations
            .iter()
            .filter_map(|decl| {
                let Expression::FunctionExpression(func) = decl.init.as_ref()? else {
                    return None;
                };
                if func.r#async {
                    return None;
                }
                Some((decl.id.span(), func.body.as_deref()?))
            })
            .collect(),
        _ => vec![],
    }
}

fn body_contains_alr_yes(body: &FunctionBody) -> bool {
    body.statements
        .iter()
        .any(|stmt| statement_has_alr_yes(stmt))
}

fn statement_has_alr_yes(stmt: &Statement) -> bool {
    match stmt {
        Statement::ExpressionStatement(expr_stmt) => call_has_alr_yes(&expr_stmt.expression),
        Statement::BlockStatement(block) => block.body.iter().any(statement_has_alr_yes),
        Statement::IfStatement(if_stmt) => {
            statement_has_alr_yes(&if_stmt.consequent)
                || if_stmt
                    .alternate
                    .as_ref()
                    .is_some_and(|alt| statement_has_alr_yes(alt))
        }
        Statement::ReturnStatement(ret) => ret
            .argument
            .as_ref()
            .is_some_and(expression_has_alr_yes),
        Statement::TryStatement(try_stmt) => {
            block_has_alr_yes(&try_stmt.block)
                || try_stmt
                    .handler
                    .as_ref()
                    .is_some_and(|h| block_has_alr_yes(&h.body))
                || try_stmt
                    .finalizer
                    .as_ref()
                    .is_some_and(|block| block_has_alr_yes(block))
        }
        Statement::SwitchStatement(switch) => switch
            .cases
            .iter()
            .any(|case| case.consequent.iter().any(statement_has_alr_yes)),
        Statement::ForStatement(for_stmt) => statement_has_alr_yes(&for_stmt.body),
        Statement::ForInStatement(for_stmt) => statement_has_alr_yes(&for_stmt.body),
        Statement::ForOfStatement(for_stmt) => statement_has_alr_yes(&for_stmt.body),
        Statement::WhileStatement(while_stmt) => statement_has_alr_yes(&while_stmt.body),
        Statement::DoWhileStatement(do_stmt) => statement_has_alr_yes(&do_stmt.body),
        Statement::WithStatement(with_stmt) => statement_has_alr_yes(&with_stmt.body),
        Statement::LabeledStatement(labeled) => statement_has_alr_yes(&labeled.body),
        _ => false,
    }
}

fn expression_has_alr_yes(expr: &Expression) -> bool {
    match expr {
        Expression::CallExpression(_) => call_has_alr_yes(expr),
        Expression::SequenceExpression(seq) => seq.expressions.iter().any(expression_has_alr_yes),
        Expression::ConditionalExpression(cond) => {
            expression_has_alr_yes(&cond.consequent) || expression_has_alr_yes(&cond.alternate)
        }
        Expression::LogicalExpression(logical) => {
            expression_has_alr_yes(&logical.left) || expression_has_alr_yes(&logical.right)
        }
        Expression::AssignmentExpression(assign) => expression_has_alr_yes(&assign.right),
        _ => false,
    }
}

fn call_has_alr_yes(expr: &Expression) -> bool {
    let Expression::CallExpression(call) = expr else {
        return false;
    };
    let Expression::StaticMemberExpression(member) = &call.callee else {
        return false;
    };
    if !matches!(member.object, Expression::Identifier(_)) {
        return false;
    }
    if call.arguments.len() != 2 {
        return false;
    }
    string_arg(&call.arguments[0]) == Some("alr") && string_arg(&call.arguments[1]) == Some("yes")
}

fn block_has_alr_yes(block: &oxc_ast::ast::BlockStatement) -> bool {
    block.body.iter().any(statement_has_alr_yes)
}

fn string_arg<'a>(arg: &'a Argument<'a>) -> Option<&'a str> {
    match arg {
        Argument::StringLiteral(lit) => Some(lit.value.as_str()),
        _ => None,
    }
}

fn create_solver(name: &str) -> String {
    format!(
        r#"({{sig,n}})=>{{const url=({name})("https://youtube.com/watch?v=yt-dlp-wins","s",sig?encodeURIComponent(sig):undefined);url.set("n",n);const proto=Object.getPrototypeOf(url);const keys=Object.keys(proto).concat(Object.getOwnPropertyNames(proto));for(const key of keys){{if(!["constructor","set","get","clone"].includes(key)){{url[key]();break;}}}}const s=url.get("s");return{{sig:s?decodeURIComponent(s):null,n:url.get("n")??null}};}}"#
    )
}

fn make_solver_fn(candidate: &SolverCandidate, param: &str) -> String {
    let solver = create_solver(&candidate.name);
    format!("({param})=>(({solver})({{{param}}})).{param}")
}

fn multi_try(generators: &[String]) -> String {
    let list = generators.join(",");
    format!(
        r#"(_input)=>{{const _results=new Set();const errors=[];for(const _generator of [{list}]){{try{{_results.add(_generator(_input));}}catch(e){{errors.push(e);}}}}if(!_results.size){{throw `no solutions: ${{errors.join(", ")}}`;}}if(_results.size!==1){{throw `invalid solutions: ${{[..._results].map(x=>JSON.stringify(x)).join(", ")}}`;}}return _results.values().next().value;}}"#
    )
}

fn source_slice(source: &str, span: Span) -> &str {
    &source[span.start as usize..span.end as usize]
}

fn eval_opts() -> rquickjs::context::EvalOptions {
    let mut opts = rquickjs::context::EvalOptions::default();
    opts.strict = false;
    opts
}

fn verify_fns(preprocessed: &str) -> Result<(), DeobfError> {
    let rt = Runtime::new()?;
    let ctx = Context::full(&rt)?;
    let testinp = crate::util::generate_content_playback_nonce();

    let js = format!("{INIT}{preprocessed}{WRAPPERS}");
    ctx.with(|ctx| {
        ctx.eval_with_options::<(), _>(js.as_bytes(), eval_opts())?;

        let sig: String = ctx
            .eval_with_options(format!("deobf_sig({testinp:?})").as_bytes(), eval_opts())
            .map_err(|e| DeobfError::Other(e.to_string().into()))?;
        if sig.is_empty() {
            return Err(DeobfError::Other(
                "deobfuscation fn returned empty string".into(),
            ));
        }

        let nsig: String = ctx
            .eval_with_options(format!("deobf_nsig({testinp:?})").as_bytes(), eval_opts())
            .map_err(|e| DeobfError::Other(e.to_string().into()))?;
        if nsig.is_empty()
            || nsig.starts_with("enhanced_except_")
            || nsig.ends_with(&testinp)
        {
            return Err(DeobfError::Other("nsig fn returned an exception".into()));
        }
        Ok(())
    })
}

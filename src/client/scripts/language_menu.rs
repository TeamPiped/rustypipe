#![cfg(test)]
use std::collections::BTreeMap;
use std::path::Path;

use fancy_regex::Regex;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::VecSkipError;

use crate::client::response::Icon;
use crate::client::{ClientType, ContextYT, RustyTube};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QLanguageMenu {
    context: ContextYT,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LanguageMenu {
    #[serde_as(as = "VecSkipError<_>")]
    actions: Vec<ActionWrap>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActionWrap {
    open_popup_action: OpenPopupAction,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenPopupAction {
    popup: Popup,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Popup {
    multi_page_menu_renderer: MultiPageMenuRenderer<MenuSectionRenderer>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MultiPageMenuRenderer<T> {
    sections: Vec<MenuSectionRendererWrap<T>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MenuSectionRendererWrap<T> {
    multi_page_menu_section_renderer: T,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MenuSectionRenderer {
    #[serde_as(as = "VecSkipError<_>")]
    items: Vec<CompactLinkRendererWrap>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompactLinkRendererWrap {
    compact_link_renderer: CompactLinkRenderer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompactLinkRenderer {
    icon: Icon,
    service_endpoint: ServiceEndpoint<MenuAction>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceEndpoint<T> {
    signal_service_endpoint: SignalServiceEndpoint<T>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignalServiceEndpoint<T> {
    actions: Vec<T>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MenuAction {
    get_multi_page_menu_action: MultiPageMenuAction,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MultiPageMenuAction {
    menu: Menu,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Menu {
    multi_page_menu_renderer: MultiPageMenuRenderer<ItemSectionRenderer>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemSectionRenderer {
    items: Vec<LanguageItemWrap>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LanguageItemWrap {
    compact_link_renderer: LanguageItem,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LanguageItem {
    #[serde_as(as = "crate::serializer::text::Text")]
    title: String,
    service_endpoint: ServiceEndpoint<LanguageCountryAction>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LanguageCountryAction {
    #[serde(alias = "selectCountryCommand")]
    select_language_command: LanguageCountryCommand,
}

#[derive(Clone, Debug, Deserialize)]
struct LanguageCountryCommand {
    #[serde(alias = "gl")]
    hl: String,
}

// #[test_log::test(tokio::test)]
#[allow(dead_code)]
async fn generate_locales() {
    let (languages, countries) = get_locales().await;

    let mut code = "// GENERATED SECTION START //\n".to_owned();

    code.push_str("#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]\n");
    code.push_str("#[serde(rename_all = \"kebab-case\")]\n");
    code.push_str("pub enum Language {\n");

    languages.iter().for_each(|(c, n)| {
        code.push_str(&format!("    /// {}\n    ", n));

        if c.contains('-') {
            code.push_str(&format!("#[serde(rename = \"{}\")]\n    ", c));
        }

        c.split('-').for_each(|c| {
            code.push_str(&format!(
                "{}{}",
                c[0..1].to_owned().to_uppercase(),
                c[1..].to_owned().to_lowercase()
            ))
        });
        code.push_str(",\n");
    });

    code.push_str("}\n\n");

    code.push_str("#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]\n");
    code.push_str("#[serde(rename_all = \"SCREAMING_SNAKE_CASE\")]\n");
    code.push_str("pub enum Country {\n");

    countries.iter().for_each(|(c, n)| {
        code.push_str(&format!("    /// {}\n", n));
        code.push_str(&format!(
            "    {}{},\n",
            c[0..1].to_owned().to_uppercase(),
            c[1..].to_owned().to_lowercase()
        ))
    });

    code.push_str("}\n");

    code.push_str("// GENERATED SECTION END //");

    let locale_path = Path::new("src/model/locale.rs");
    let src = std::fs::read_to_string(locale_path).unwrap();

    let delim_pattern =
        Regex::new("// GENERATED SECTION START //\n[^@]*// GENERATED SECTION END //").unwrap();

    let new_src = delim_pattern.replace(&src, code);
    std::fs::write(locale_path, new_src.as_bytes()).unwrap();
}

async fn get_locales() -> (BTreeMap<String, String>, BTreeMap<String, String>) {
    let rt = RustyTube::new();
    let client = rt.get_ytclient(ClientType::Desktop);
    let context = client.get_context(true).await;

    let request_body = QLanguageMenu { context };

    let resp = client
        .request_builder(Method::POST, "account/account_menu")
        .await
        .json(&request_body)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let language_menu = resp.json::<LanguageMenu>().await.unwrap();

    let lm_section = &language_menu.actions[0]
        .open_popup_action
        .popup
        .multi_page_menu_renderer
        .sections
        .iter()
        .find(|s| s.multi_page_menu_section_renderer.items.len() >= 2)
        .unwrap();

    let lang_section = lm_section
        .multi_page_menu_section_renderer
        .items
        .iter()
        .find(|s| s.compact_link_renderer.icon.icon_type == "TRANSLATE")
        .unwrap();

    let country_section = lm_section
        .multi_page_menu_section_renderer
        .items
        .iter()
        .find(|s| s.compact_link_renderer.icon.icon_type == "LANGUAGE")
        .unwrap();

    let languages = map_language_section(lang_section);
    let countries = map_language_section(country_section);

    (languages, countries)
}

fn map_language_section(section: &CompactLinkRendererWrap) -> BTreeMap<String, String> {
    section
        .compact_link_renderer
        .service_endpoint
        .signal_service_endpoint
        .actions[0]
        .get_multi_page_menu_action
        .menu
        .multi_page_menu_renderer
        .sections[0]
        .multi_page_menu_section_renderer
        .items
        .iter()
        .map(|i| {
            (
                i.compact_link_renderer
                    .service_endpoint
                    .signal_service_endpoint
                    .actions[0]
                    .select_language_command
                    .hl
                    .to_owned(),
                i.compact_link_renderer.title.to_owned(),
            )
        })
        .collect::<BTreeMap<_, _>>()
}

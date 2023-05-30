const fs = require("fs");

const DICT_PATH = "../dictionary.json";

function translateLang(lang) {
  switch (lang) {
    case "iw": // Hebrew
      return "he";
    case "zh-CN": // Simplified Chinese
      return "zh-Hans";
    case "zh-HK":
      return "zh-Hant-HK";
    case "zh-TW":
      return "zh-Hant";
    default:
      return lang;
  }
}

function prepString(s, by_char) {
  const replaced = s.toLowerCase().replace("{0}", "").replace("-", " ");
  if (by_char) {
    return replaced.replace(/\s/, "").split("");
  } else {
    return replaced.split(/\s+/);
  }
}

function storeToken(tokens, word, unit) {
  if (word) {
    if (word in tokens && tokens[word] != unit) {
      tokens[word] = null;
    } else {
      tokens[word] = unit;
    }
  }
}

function validateTokens(tokens, lang) {
  const units = { Y: 1, M: 1, W: 1, D: 1, h: 1, m: 1, s: 1 };

  if (lang === "iw") {
    tokens["שתי"] = "2";
  }

  for (const [key, val] of Object.entries(tokens)) {
    if (val === null) {
      delete tokens[key];
    } else {
      delete units[val];
    }
  }
  if (Object.keys(units).length > 0) {
    console.log(
      `missing units ${JSON.stringify(
        Object.keys(units)
      )} for lang: ${lang}; tokens: ${JSON.stringify(tokens)}`
    );
  }
}

function validateNdTokens(tokens, lang) {
  const units = { "0D": 1, "1D": 1 };

  for (const [key, val] of Object.entries(tokens)) {
    if (val === null) {
      delete tokens[key];
    } else {
      delete units[val];
    }
  }

  if (Object.keys(units).length > 0) {
    console.log(
      `missing nd tokens ${JSON.stringify(
        Object.keys(units)
      )} for lang: ${lang}; tokens: ${JSON.stringify(tokens)}`
    );
  } else if (Object.keys(tokens).length > 2) {
    console.log(
      `too many nd tokens for lang: ${lang}; tokens: ${JSON.stringify(tokens)}`
    );
  }
}

const sortObject = (obj) =>
  Object.keys(obj)
    .sort()
    .reduce((res, key) => ((res[key] = obj[key]), res), {});

function collectTimeago(lang, by_char, timeagoTokens, timeagoNdTokens) {
  const cldrLang = translateLang(lang);
  const dates = require(`cldr-dates-modern/main/${cldrLang}/dateFields.json`);
  const dateFields = dates.main[cldrLang].dates.fields;

  for (const [unitStr, unit] of Object.entries(units)) {
    for (const unitFields of [dateFields[unitStr], dateFields[`${unitStr}-short`]]) {
      for (const [sKey, s] of Object.entries(unitFields["relativeTime-type-past"])) {
        let u = unit;
        if (s.indexOf("{0}") === -1) {
          if (sKey.endsWith("-zero")) {
            u = "0" + u;
          } else if (sKey.endsWith("-one")) {
            u = "1" + u;
          } else if (sKey.endsWith("-two")) {
            u = "2" + u;
          } else {
            throw new Error(`Invalid time pattern. lang: ${lang} key: ${sKey}`);
          }
        }

        const words = prepString(s, by_char);
        for (const word of words) {
          storeToken(timeagoTokens, word, u);
        }
      }
    }
  }

  if (dateFields.day["relative-type-0"]) {
    const words = prepString(dateFields.day["relative-type-0"], by_char);
    for (const word of words) {
      storeToken(timeagoNdTokens, word, "0D");
    }
  }
  if (dateFields.day["relative-type--1"]) {
    const words = prepString(dateFields.day["relative-type--1"], by_char);
    for (const word of words) {
      storeToken(timeagoNdTokens, word, "1D");
    }
  }
}

const dict = JSON.parse(fs.readFileSync(DICT_PATH));

const units = {
  second: "s",
  minute: "m",
  hour: "h",
  day: "D",
  week: "W",
  month: "M",
  year: "Y",
};

for (const [mainLang, entry] of Object.entries(dict)) {
  const langs = [mainLang, ...entry["equivalent"]];

  const timeagoTokens = {};
  const timeagoNdTokens = {};

  for (lang of langs) {
    collectTimeago(lang, entry["by_char"], timeagoTokens, timeagoNdTokens);
  }
  validateTokens(timeagoTokens, mainLang);
  // validateNdTokens(timeagoNdTokens, mainLang);

  dict[mainLang]["timeago_tokens"] = timeagoTokens;
  // dict[mainLang]["timeago_nd_tokens"] = timeagoNdTokens;
}

fs.writeFileSync(DICT_PATH, JSON.stringify(dict, null, 2));

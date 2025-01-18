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

function collectMonthNames(lang, by_char, monthNames, monthShortNames, weekdayNames) {
  const cldrLang = translateLang(lang);
  const dates = require(`cldr-dates-modern/main/${cldrLang}/ca-gregorian.json`);
  const dateFields = dates.main[cldrLang].dates.calendars.gregorian;

  const months = dateFields.months["stand-alone"].wide;

  // Mongolian dates have the extra numbe арван and have to be handled manually
  if (!["mn"].includes(lang)) {
    for (const [n, name] of Object.entries(months)) {
      let name2 = name.toLowerCase();
      if (name2.includes(n)) {
        // Some languages dont have named months
        console.log(`${lang}: month name '${name2}' includes number; skipped`);
        continue;
      }
      if (/\s/g.test(name2)) {
        throw new Error(`${lang}: month name '${name2}' contains whitespace`);
      }
      monthNames[name2] = parseInt(n);
    }
  }

  if (!["bg", "fi", "cs", "iw", "lt", "pt-PT", "sk"].includes(lang)) {
    const monthsShort = dateFields.months.format.abbreviated;
    for (const [n, name] of Object.entries(monthsShort)) {
      let name2 = name.toLowerCase().replaceAll(".", "");
      if (name2.includes(n)) {
        // Some languages dont have named months
        console.log(`${lang}: month name '${name2}' includes number; skipped`);
        continue;
      }
      if (lang === "ca") {
        name2 = name2.replace("de ", "");
      }
      if (/\s/g.test(name2)) {
        throw new Error(`${lang}: month name '${name2}' contains whitespace`);
      }
      monthShortNames[name2] = parseInt(n);
    }
  }

  const weekdays = dateFields.days["stand-alone"].wide;
  for (const [id, name] of Object.entries(weekdays)) {
    let name2 = name.toLowerCase();

    if (by_char) {
      name2 = name2.replace("曜日", "").replace("星期", "");
      if (name2.length != 1) {
        throw new Error(`${lang}: single-char name '${name2}' has invalid length`);
      }
    } else {
      if (lang === "iw") {
        name2 = name2.replace("יום ", "");
      } else if (lang === "sq") {
        name2 = name2.replace("e ", "");
      }
      if (/\s/g.test(name2)) {
        // throw new Error(`${lang}: name '${name2}' contains whitespace`);
        console.log(`${lang}: weekday name '${name2}' contains whitespace`);
      }
    }

    const ids = { mon: 0, tue: 1, wed: 2, thu: 3, fri: 4, sat: 5, sun: 6 };
    const n = ids[id];
    weekdayNames[name2] = `${n}Wd`;
  }
}

const dict = JSON.parse(fs.readFileSync(DICT_PATH));

for (const [mainLang, entry] of Object.entries(dict)) {
  const langs = [mainLang, ...entry["equivalent"]];
  let monthNames = {};
  let monthShortNames = {};
  let weekdayNames = {};

  for (lang of langs) {
    collectMonthNames(
      lang,
      entry["by_char"],
      monthNames,
      monthShortNames,
      weekdayNames
    );
  }
  dict[mainLang]["months"] = {
    ...dict[mainLang]["months"],
    ...monthNames,
    ...monthShortNames,
  };
  dict[mainLang]["timeago_nd_tokens"] = {
    ...dict[mainLang]["timeago_nd_tokens"],
    ...weekdayNames,
  };
}

fs.writeFileSync(DICT_PATH, JSON.stringify(dict, null, 2));

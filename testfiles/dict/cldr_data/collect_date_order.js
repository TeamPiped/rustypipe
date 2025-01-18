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

function isMonthBeforeDay(lang) {
  const cldrLang = translateLang(lang);
  const dates = require(`cldr-dates-modern/main/${cldrLang}/ca-gregorian.json`);
  const dateFields = dates.main[cldrLang].dates.calendars.gregorian;

  const dateFmt = dateFields.dateFormats.short;
  const mPos = dateFmt.indexOf("M");
  const dPos = dateFmt.indexOf("d");
  if (mPos < 0 || dPos < 0) throw new Error(`invalid fmt for ${lang}: ${dateFmt}`);
  return dPos > mPos;
}

const dict = JSON.parse(fs.readFileSync(DICT_PATH));

for (const [mainLang, entry] of Object.entries(dict)) {
  const langs = [mainLang, ...entry["equivalent"]];
  const dateOrder = entry["date_order"];
  const mPos = dateOrder.indexOf("M");
  const dPos = dateOrder.indexOf("D");
  let expectMbd = mPos < 0 || dPos < 0 ? null : dPos > mPos;

  if (mainLang === "en" || mainLang.startsWith("zh-")) {
    expectMbd = true;
  } else if (mainLang === "fr")
    expectMbd = false;
  else {
    for (lang of langs) {
      const mbd = isMonthBeforeDay(lang)
      if (expectMbd === null) {
        expectMbd = mbd;
      } else if (mbd !== expectMbd) {
        throw new Error(`unexpected mbd for ${lang}`);
      }
    }
  }

  dict[mainLang]["month_before_day"] = expectMbd;
}

fs.writeFileSync(DICT_PATH, JSON.stringify(dict, null, 2));

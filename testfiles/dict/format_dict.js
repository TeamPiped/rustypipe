const fs = require("fs");
const DICT_PATH = "dictionary.json";

const dict = JSON.parse(fs.readFileSync(DICT_PATH));

const TA_TOKENS = { s: 1, m: 2, h: 3, D: 4, W: 5, M: 6, Y: 7 };

// Sort a dictionary by value
function sortDict(dict) {
  // Create items array
  const items = Object.keys(dict).map(function (key) {
    return [key, dict[key]];
  });
  // Sort the array based on the second element
  items.sort(function (first, second) {
    let d1;
    if (typeof first[1] === "string") {
      // Check for timeago token
      const tokenPat = /^(\d*)([smhDWMY])$/;
      const m1 = first[1].match(tokenPat);
      const m2 = second[1].match(tokenPat);
      if (m1 !== null && m2 !== null) {
        const tok1 = TA_TOKENS[m1[2]];
        const tok2 = TA_TOKENS[m2[2]];
        if (tok1 === tok2) {
          const n1 = parseInt(m1[1]) || 0;
          const n2 = parseInt(m2[1]) || 0;
          d1 = n1 - n2;
        } else {
          d1 = tok1 - tok2;
        }
      } else {
        d1 = first[1].localeCompare(second[1]);
      }
    } else {
      d1 = first[1] - second[1];
    }
    if (d1 === 0) {
      return first[0].localeCompare(second[0]);
    }
    return d1;
  });

  const newDict = {};
  items.forEach((item) => {
    newDict[item[0]] = item[1];
  });
  return newDict;
}

for (const [mainLang, entry] of Object.entries(dict)) {
  for (key of Object.keys(entry)) {
    const val = entry[key];
    if (typeof val === "object" && !Array.isArray(val)) {
      dict[mainLang][key] = sortDict(val);
    }
  }
}

fs.writeFileSync(DICT_PATH, JSON.stringify(dict, null, 2) + "\n");

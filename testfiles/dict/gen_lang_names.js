const fs = require("fs");

const dict = JSON.parse(fs.readFileSync("dictionary.json"));

const intl = new Intl.DisplayNames(["en"], { type: "language" });

let langs = Object.keys(dict);
Object.values(dict).forEach(entry => {
  if (entry.equivalent) {
    langs.push(...entry.equivalent);
  }
});
langs.sort();

const res = Object.fromEntries(langs.map((l) => [l, intl.of(l)]));
fs.writeFileSync("lang_names.json", JSON.stringify(res, null, 2));

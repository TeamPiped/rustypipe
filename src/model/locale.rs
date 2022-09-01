use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};

// GENERATED SECTION START //
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum Language {
    /// Afrikaans
    Af,
    /// አማርኛ
    Am,
    /// العربية
    Ar,
    /// অসমীয়া
    As,
    /// Azərbaycan
    Az,
    /// Беларуская
    Be,
    /// Български
    Bg,
    /// বাংলা
    Bn,
    /// Bosanski
    Bs,
    /// Català
    Ca,
    /// Čeština
    Cs,
    /// Dansk
    Da,
    /// Deutsch
    De,
    /// Ελληνικά
    El,
    /// English (US)
    En,
    /// English (UK)
    #[serde(rename = "en-GB")]
    EnGb,
    /// English (India)
    #[serde(rename = "en-IN")]
    EnIn,
    /// Español (España)
    Es,
    /// Español (Latinoamérica)
    #[serde(rename = "es-419")]
    Es419,
    /// Español (US)
    #[serde(rename = "es-US")]
    EsUs,
    /// Eesti
    Et,
    /// Euskara
    Eu,
    /// فارسی
    Fa,
    /// Suomi
    Fi,
    /// Filipino
    Fil,
    /// Français
    Fr,
    /// Français (Canada)
    #[serde(rename = "fr-CA")]
    FrCa,
    /// Galego
    Gl,
    /// ગુજરાતી
    Gu,
    /// हिन्दी
    Hi,
    /// Hrvatski
    Hr,
    /// Magyar
    Hu,
    /// Հայերեն
    Hy,
    /// Bahasa Indonesia
    Id,
    /// Íslenska
    Is,
    /// Italiano
    It,
    /// עברית
    Iw,
    /// 日本語
    Ja,
    /// ქართული
    Ka,
    /// Қазақ Тілі
    Kk,
    /// ខ្មែរ
    Km,
    /// ಕನ್ನಡ
    Kn,
    /// 한국어
    Ko,
    /// Кыргызча
    Ky,
    /// ລາວ
    Lo,
    /// Lietuvių
    Lt,
    /// Latviešu valoda
    Lv,
    /// Македонски
    Mk,
    /// മലയാളം
    Ml,
    /// Монгол
    Mn,
    /// मराठी
    Mr,
    /// Bahasa Malaysia
    Ms,
    /// ဗမာ
    My,
    /// नेपाली
    Ne,
    /// Nederlands
    Nl,
    /// Norsk
    No,
    /// ଓଡ଼ିଆ
    Or,
    /// ਪੰਜਾਬੀ
    Pa,
    /// Polski
    Pl,
    /// Português (Brasil)
    Pt,
    /// Português
    #[serde(rename = "pt-PT")]
    PtPt,
    /// Română
    Ro,
    /// Русский
    Ru,
    /// සිංහල
    Si,
    /// Slovenčina
    Sk,
    /// Slovenščina
    Sl,
    /// Shqip
    Sq,
    /// Српски
    Sr,
    /// Srpski
    #[serde(rename = "sr-Latn")]
    SrLatn,
    /// Svenska
    Sv,
    /// Kiswahili
    Sw,
    /// தமிழ்
    Ta,
    /// తెలుగు
    Te,
    /// ภาษาไทย
    Th,
    /// Türkçe
    Tr,
    /// Українська
    Uk,
    /// اردو
    Ur,
    /// O‘zbek
    Uz,
    /// Tiếng Việt
    Vi,
    /// 中文 (简体)
    #[serde(rename = "zh-CN")]
    ZhCn,
    /// 中文 (香港)
    #[serde(rename = "zh-HK")]
    ZhHk,
    /// 中文 (繁體)
    #[serde(rename = "zh-TW")]
    ZhTw,
    /// IsiZulu
    Zu,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Country {
    /// United Arab Emirates
    Ae,
    /// Argentina
    Ar,
    /// Austria
    At,
    /// Australia
    Au,
    /// Azerbaijan
    Az,
    /// Bosnia and Herzegovina
    Ba,
    /// Bangladesh
    Bd,
    /// Belgium
    Be,
    /// Bulgaria
    Bg,
    /// Bahrain
    Bh,
    /// Bolivia
    Bo,
    /// Brazil
    Br,
    /// Belarus
    By,
    /// Canada
    Ca,
    /// Switzerland
    Ch,
    /// Chile
    Cl,
    /// Colombia
    Co,
    /// Costa Rica
    Cr,
    /// Cyprus
    Cy,
    /// Czechia
    Cz,
    /// Germany
    De,
    /// Denmark
    Dk,
    /// Dominican Republic
    Do,
    /// Algeria
    Dz,
    /// Ecuador
    Ec,
    /// Estonia
    Ee,
    /// Egypt
    Eg,
    /// Spain
    Es,
    /// Finland
    Fi,
    /// France
    Fr,
    /// United Kingdom
    Gb,
    /// Georgia
    Ge,
    /// Ghana
    Gh,
    /// Greece
    Gr,
    /// Guatemala
    Gt,
    /// Hong Kong
    Hk,
    /// Honduras
    Hn,
    /// Croatia
    Hr,
    /// Hungary
    Hu,
    /// Indonesia
    Id,
    /// Ireland
    Ie,
    /// Israel
    Il,
    /// India
    In,
    /// Iraq
    Iq,
    /// Iceland
    Is,
    /// Italy
    It,
    /// Jamaica
    Jm,
    /// Jordan
    Jo,
    /// Japan
    Jp,
    /// Kenya
    Ke,
    /// Cambodia
    Kh,
    /// South Korea
    Kr,
    /// Kuwait
    Kw,
    /// Kazakhstan
    Kz,
    /// Laos
    La,
    /// Lebanon
    Lb,
    /// Liechtenstein
    Li,
    /// Sri Lanka
    Lk,
    /// Lithuania
    Lt,
    /// Luxembourg
    Lu,
    /// Latvia
    Lv,
    /// Libya
    Ly,
    /// Morocco
    Ma,
    /// Montenegro
    Me,
    /// North Macedonia
    Mk,
    /// Malta
    Mt,
    /// Mexico
    Mx,
    /// Malaysia
    My,
    /// Nigeria
    Ng,
    /// Nicaragua
    Ni,
    /// Netherlands
    Nl,
    /// Norway
    No,
    /// Nepal
    Np,
    /// New Zealand
    Nz,
    /// Oman
    Om,
    /// Panama
    Pa,
    /// Peru
    Pe,
    /// Papua New Guinea
    Pg,
    /// Philippines
    Ph,
    /// Pakistan
    Pk,
    /// Poland
    Pl,
    /// Puerto Rico
    Pr,
    /// Portugal
    Pt,
    /// Paraguay
    Py,
    /// Qatar
    Qa,
    /// Romania
    Ro,
    /// Serbia
    Rs,
    /// Russia
    Ru,
    /// Saudi Arabia
    Sa,
    /// Sweden
    Se,
    /// Singapore
    Sg,
    /// Slovenia
    Si,
    /// Slovakia
    Sk,
    /// Senegal
    Sn,
    /// El Salvador
    Sv,
    /// Thailand
    Th,
    /// Tunisia
    Tn,
    /// Turkey
    Tr,
    /// Taiwan
    Tw,
    /// Tanzania
    Tz,
    /// Ukraine
    Ua,
    /// Uganda
    Ug,
    /// United States
    Us,
    /// Uruguay
    Uy,
    /// Venezuela
    Ve,
    /// Vietnam
    Vn,
    /// Yemen
    Ye,
    /// South Africa
    Za,
    /// Zimbabwe
    Zw,
}
// GENERATED SECTION END //

impl Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            &serde_json::to_string(self).map_or("".to_owned(), |s| s[1..s.len() - 1].to_owned()),
        )
    }
}

impl Display for Country {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            &serde_json::to_string(self).map_or("".to_owned(), |s| s[1..s.len() - 1].to_owned()),
        )
    }
}

impl FromStr for Language {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(&format!("\"{}\"", s))
    }
}

impl FromStr for Country {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(&format!("\"{}\"", s))
    }
}

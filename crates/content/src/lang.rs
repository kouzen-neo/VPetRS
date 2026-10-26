//! Localization string lookup (`lang/<locale>/*.lps` and `info.lps` `lang#<locale>` lines).

use std::collections::BTreeMap;

use crate::lps::LpsDocument;

#[derive(Debug, Clone, Default)]
pub struct LangTable {
    locales: BTreeMap<String, BTreeMap<String, String>>,
}

impl LangTable {
    pub fn load_locale_doc(&mut self, locale: &str, doc: &LpsDocument) {
        let map = self.locales.entry(locale.to_ascii_lowercase()).or_default();
        for line in &doc.lines {
            if !line.name.is_empty() && !line.info.is_empty() {
                map.insert(line.name.clone(), line.info.clone());
            }
            for sub in &line.subs {
                if !sub.name.is_empty() && !sub.info.is_empty() {
                    map.insert(sub.name.clone(), sub.info.clone());
                }
            }
        }
    }

    pub fn load_mod_info_doc(&mut self, doc: &LpsDocument) {
        for line in &doc.lines {
            if line.name.eq_ignore_ascii_case("lang") && !line.info.is_empty() {
                let map = self
                    .locales
                    .entry(line.info.to_ascii_lowercase())
                    .or_default();
                for sub in &line.subs {
                    if !sub.name.is_empty() && !sub.info.is_empty() {
                        map.insert(sub.name.clone(), sub.info.clone());
                    }
                }
            }
        }
    }

    #[must_use]
    pub fn translate<'a>(&'a self, locale: &str, key: &'a str) -> &'a str {
        let loc = locale.to_ascii_lowercase();
        if let Some(table) = self.locales.get(&loc)
            && let Some(val) = table.get(key)
        {
            return val.as_str();
        }
        if let Some((prefix, _)) = loc.split_once('-')
            && let Some(table) = self.locales.get(prefix)
            && let Some(val) = table.get(key)
        {
            return val.as_str();
        }
        key
    }

    #[must_use]
    pub fn locale_count(&self, locale: &str) -> usize {
        self.locales
            .get(&locale.to_ascii_lowercase())
            .map_or(0, BTreeMap::len)
    }
}

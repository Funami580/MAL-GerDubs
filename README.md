# MAL (MyAnimeList) German Dubs
[![Install with Tampermonkey](https://img.shields.io/badge/Install%20directly%20with-Tampermonkey-00485b?logo=tampermonkey)](https://raw.githubusercontent.com/Funami580/MAL-GerDubs/main/mal-dubs.user.js)
[![GitHub tag (latest SemVer)](https://img.shields.io/github/v/tag/Funami580/MAL-GerDubs?label=Version)](#mal-myanimelist-german-dubs)
[![GitHub license](https://img.shields.io/github/license/Funami580/MAL-GerDubs)](https://github.com/Funami580/MAL-GerDubs/blob/main/LICENSE)

MAL Dubs is a userscript which labels more than 2000 German dubbed titles on MyAnimeList.net and adds a "Dub Only" filter to search, seasonal, company, top anime pages and your personal anime list.

Whether you watch dubs because you like to multitask while watching, because you have a visual impairment that makes subtitles difficult to read, or because you prefer hearing a performance in a language you speak, it can be hard to use MyAnimeList to find anime dubbed in German. This userscript fixes that problem.

This project is basically a copy of the English version with some slight adjustments: [MAL-Dubs](https://github.com/MAL-Dubs/MAL-Dubs)

![Look for the "D"](https://raw.githubusercontent.com/MAL-Dubs/MAL-Dubs/main/images/labels.png)
![Find just the dubs ☑](https://raw.githubusercontent.com/MAL-Dubs/MAL-Dubs/main/images/filter.png)

## Instructions

**Step 1: Install the [Tampermonkey](https://www.tampermonkey.net/) Script Manager extension**
- [Chrome](https://chrome.google.com/webstore/detail/dhdgffkkebhmkfjojejmpbldmpobfkfo)
- [Firefox](https://addons.mozilla.org/en-US/firefox/addon/tampermonkey/)
- [Safari](https://apps.apple.com/app/apple-store/id1482490089)
- [Opera](https://addons.opera.com/en/extensions/details/tampermonkey-beta/)
- [Edge](https://microsoftedge.microsoft.com/addons/detail/iikmkjmpaadaobahmlepeloendndfphd)

**Step 2: Install the script**

[Click here to install](https://raw.githubusercontent.com/Funami580/MAL-GerDubs/main/mal-dubs.user.js)

## Update Data
Clone the repository, if you have not already:
```
git clone --depth 1 https://github.com/Funami580/MAL-GerDubs.git
```

Otherwise, follow the normal procedure:
```
cd MAL-GerDubs
git submodule update --init --recursive --remote
cd gen_data
cargo build --release
./target/release/mal_gerdubs
```

## Support the Parent Project

Quote from [MAL-Dubs](https://github.com/MAL-Dubs/MAL-Dubs) (English version)

> I personally add each title whenever I see a new dub announcement. Your support will help me to keep you up to date!

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/Y8Y21HXGO)

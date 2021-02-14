use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, std::fmt::Debug)]
pub struct SearchResponse {
    #[serde(rename = "batchcomplete")]
    pub batchcomplete: String,

    #[serde(rename = "continue")]
    pub search_response_continue: Option<Continue>,

    #[serde(rename = "query")]
    pub query: Query,
}

#[derive(Serialize, Deserialize, std::fmt::Debug)]
pub struct Query {
    #[serde(rename = "searchinfo")]
    pub searchinfo: Searchinfo,

    #[serde(rename = "search")]
    pub search: Vec<Search>,
}

#[derive(Serialize, Deserialize, std::fmt::Debug, Clone)]
pub struct Search {
    #[serde(rename = "ns")]
    pub ns: i64,

    #[serde(rename = "title")]
    pub title: String,

    #[serde(rename = "pageid")]
    pub pageid: i64,

    #[serde(rename = "size")]
    pub size: i64,

    #[serde(rename = "wordcount")]
    pub wordcount: i64,

    #[serde(rename = "snippet")]
    pub snippet: String,

    #[serde(rename = "timestamp")]
    pub timestamp: String,
}

#[derive(Serialize, Deserialize, std::fmt::Debug)]
pub struct Searchinfo {
    #[serde(rename = "totalhits")]
    pub totalhits: i64,

    #[serde(rename = "suggestion")]
    pub suggestion: Option<String>,

    #[serde(rename = "suggestionsnippet")]
    pub suggestionsnippet: Option<String>,
}

#[derive(Serialize, Deserialize, std::fmt::Debug)]
pub struct Continue {
    #[serde(rename = "sroffset")]
    pub sroffset: i64,

    #[serde(rename = "continue")]
    pub continue_continue: String,
}

#[derive(Serialize, Deserialize, std::fmt::Debug, Clone)]
pub struct HtmlPageResult {
    pub parse: Parse,
}

#[derive(Serialize, Deserialize, std::fmt::Debug,Clone)]
pub struct Parse {
    pub title: String,
    pub pageid: i64,
    pub text: String,
}

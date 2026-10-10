use serde::{Deserialize, Serialize};

use super::super::Subject;
use crate::CONFIG;
use crate::utils::text::truncate;

/// 搜索接口一次最多返回多少条：请求更大的 `limit`（试过 50）也只回 20 条，是接口侧截断。
pub const MAX_LIMIT: u32 = 20;

/// 调用方没指定 `limit` 时给多少条（`aki-cli` 的 `--limit` 默认值也是它）。
pub const DEFAULT_LIMIT: u32 = 10;

/// 错误响应里附带的响应片段长度上限。
const ERROR_BODY_LIMIT: usize = 200;

/// 请求 Bangumi 搜索条目时产生的错误。
#[derive(Debug, thiserror::Error)]
pub enum SearchSubjectError {
    #[error("bangumi search request error")]
    Request(#[source] reqwest::Error),
    #[error("bangumi returned status {status} for '{url}': {body}")]
    Status {
        status: reqwest::StatusCode,
        url: String,
        body: String,
    },
    #[error(
        "bangumi search response parse error for '{url}': {source}; response starts with: {body}"
    )]
    Parse {
        #[source]
        source: serde_json::Error,
        url: String,
        body: String,
    },
}

/// `POST /v0/search/subjects` 的请求体。
///
/// 分页参数（`limit` / `offset`）走 query string，不放进请求体，因此标了 `skip`。
#[derive(Debug, Clone, Serialize)]
pub struct SearchSubjects {
    /// 检索关键词，必填。
    pub keyword: String,
    /// 排序规则，不给则由接口按匹配程度排。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<SearchSort>,
    /// 筛选条件。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<SearchSubjectsFilter>,
    #[serde(skip)]
    pub limit: u32,
    #[serde(skip)]
    pub offset: u32,
}

impl SearchSubjects {
    /// 只给关键词，其余用默认值（[`DEFAULT_LIMIT`] 条、从第 0 条开始、不筛选）。
    #[must_use]
    pub fn new(keyword: impl Into<String>) -> Self {
        Self {
            keyword: keyword.into(),
            ..Self::default()
        }
    }
}

impl Default for SearchSubjects {
    fn default() -> Self {
        Self {
            keyword: String::new(),
            sort: None,
            filter: None,
            limit: DEFAULT_LIMIT,
            offset: 0,
        }
    }
}

/// 排序规则。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchSort {
    /// 按匹配程度（meilisearch 默认排序）。
    #[default]
    Match,
    /// 按收藏人数。
    Heat,
    /// 按排名由高到低。
    Rank,
    /// 按评分。
    Score,
}

impl SearchSort {
    /// 接口里用的字符串，与 serde 的序列化结果一致。
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Match => "match",
            Self::Heat => "heat",
            Self::Rank => "rank",
            Self::Score => "score",
        }
    }
}

/// 搜索筛选条件。**不同字段之间是「且」**，同一字段的多个值是「或」（`type`）
/// 或「且」（其余），详见接口文档。
///
/// 所有字段都可省略，省略的字段不会出现在请求体里（不是发 `null`）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchSubjectsFilter {
    /// 条目类型，多值之间为「或」。
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<i64>>,
    /// 标签，多值之间为「且」。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<Vec<String>>,
    /// 公共标签，多值之间为「且」；`-标签` 表示排除。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta_tags: Option<Vec<String>>,
    /// 播出/发售日期，`YYYY-MM-DD`，可带比较符，如 `>=2020-07-01`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub air_date: Option<Vec<String>>,
    /// 评分区间，如 `>=6`、`<8`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<Vec<String>>,
    /// 评分人数区间，格式同 `rating`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating_count: Option<Vec<String>>,
    /// 排名区间，如 `>10`、`<=18`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<Vec<String>>,
    /// `true` 只要 R18 条目，`false` 只要非 R18；不给则不发送该字段。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nsfw: Option<bool>,
}

/// 搜索结果的一页（接口文档里的 `Paged[Subject]`）。
#[derive(Debug, Clone, Deserialize)]
pub struct SubjectSearchResult {
    /// 命中的总条数，不受 `limit` 影响。
    pub total: i64,
    /// 本次实际返回的条数上限（接口会把过大的 `limit` 截到 [`MAX_LIMIT`]）。
    pub limit: i64,
    pub offset: i64,
    /// 命中的条目，字段与 `/v0/subjects/{id}` 的返回一致。
    pub data: Vec<Subject>,
}

/// 按关键词搜索条目。
///
/// 只做请求与解析，不落盘、也不打印——调用方（CLI）决定怎么展示。
///
/// # Errors
/// 网络失败、非 2xx 状态码、响应不是合法的 `Paged[Subject]` 时返回错误。
pub async fn search_subjects(
    client: &reqwest::Client,
    request: &SearchSubjects,
) -> Result<SubjectSearchResult, SearchSubjectError> {
    let url = format!("{}/v0/search/subjects", CONFIG.bangumi.bangumi_api);
    let response = client
        .post(&url)
        .query(&[("limit", request.limit), ("offset", request.offset)])
        .header(reqwest::header::USER_AGENT, "Griklit/aki")
        .json(request)
        .send()
        .await
        .map_err(SearchSubjectError::Request)?;
    let status = response.status();
    let body = response.text().await.map_err(SearchSubjectError::Request)?;
    if !status.is_success() {
        return Err(SearchSubjectError::Status {
            status,
            url,
            body: truncate(&body, ERROR_BODY_LIMIT),
        });
    }
    serde_json::from_str(&body).map_err(|source| SearchSubjectError::Parse {
        source,
        url,
        body: truncate(&body, ERROR_BODY_LIMIT),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 省略的字段不能出现在请求体里：接口把 `null` 和「没给」当成两种意思。
    #[test]
    fn omitted_filter_fields_are_not_serialized() {
        let request = SearchSubjects::new("物语");
        let body = serde_json::to_value(&request).unwrap();
        assert_eq!(body, serde_json::json!({ "keyword": "物语" }));
    }

    /// 分页参数走 query string，不能混进请求体。
    #[test]
    fn paging_is_not_serialized() {
        let request = SearchSubjects {
            limit: 5,
            offset: 10,
            ..SearchSubjects::new("物语")
        };
        let body = serde_json::to_value(&request).unwrap();
        assert!(body.get("limit").is_none());
        assert!(body.get("offset").is_none());
        assert_eq!(request.limit, 5);
        assert_eq!(request.offset, 10);
    }

    #[test]
    fn filter_and_sort_use_the_api_spelling() {
        let request = SearchSubjects {
            sort: Some(SearchSort::Heat),
            filter: Some(SearchSubjectsFilter {
                types: Some(vec![2]),
                nsfw: Some(true),
                ..SearchSubjectsFilter::default()
            }),
            ..SearchSubjects::new("物语")
        };
        let body = serde_json::to_value(&request).unwrap();
        assert_eq!(body["sort"], serde_json::json!("heat"));
        assert_eq!(body["filter"]["type"], serde_json::json!([2]));
        assert_eq!(body["filter"]["nsfw"], serde_json::json!(true));
        assert!(body["filter"].get("tag").is_none());
        assert!(body["filter"].get("rank").is_none());
    }

    /// 真实的搜索响应（完整 `Subject` + 事件性的 `image` 字段）必须能解析。
    #[test]
    fn parses_a_search_response() {
        let json = r#"{
            "data": [{
                "id": 1988,
                "type": 1,
                "name": "〈物語〉シリーズ",
                "name_cn": "物语系列",
                "summary": "summary",
                "series": true,
                "nsfw": false,
                "locked": false,
                "date": "",
                "platform": "小说",
                "image": "https://lain.bgm.tv/pic/cover/l/9d/73/1988_xp6Kw.jpg",
                "images": {"small": "s", "grid": "g", "large": "l", "medium": "m", "common": "c"},
                "infobox": [],
                "volumes": 0,
                "eps": 0,
                "total_episodes": 0,
                "rating": {"rank": 12, "total": 4532, "count": {"1": 0}, "score": 8.5},
                "collection": {"wish": 1, "collect": 2, "doing": 3, "on_hold": 4, "dropped": 5},
                "meta_tags": [],
                "tags": [{"name": "西尾維新", "count": 670, "total_count": 2798}]
            }],
            "total": 13485,
            "limit": 20,
            "offset": 0
        }"#;
        let result: SubjectSearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.total, 13_485);
        assert_eq!(result.limit, 20);
        assert_eq!(result.data.len(), 1);
        assert_eq!(result.data[0].name_cn, "物语系列");
        assert!(result.data[0].date.is_none(), "空日期应当当成没有日期");
        assert_eq!(
            result.data[0].r#type_enumed().unwrap(),
            crate::source::bangumi::SubjectType::Book
        );
    }

    /// 空结果也是合法响应，不能报错。
    #[test]
    fn parses_an_empty_response() {
        let result: SubjectSearchResult =
            serde_json::from_str(r#"{"data":[],"total":0,"limit":20,"offset":0}"#).unwrap();
        assert_eq!(result.total, 0);
        assert!(result.data.is_empty());
    }

    /// `as_str()` 必须和 serde 序列化结果一致，否则请求体会和文档对不上。
    #[test]
    fn sort_as_str_agrees_with_serde() {
        for sort in [
            SearchSort::Match,
            SearchSort::Heat,
            SearchSort::Rank,
            SearchSort::Score,
        ] {
            assert_eq!(
                serde_json::to_value(sort).unwrap(),
                serde_json::json!(sort.as_str())
            );
        }
    }
}

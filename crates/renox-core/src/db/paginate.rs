use std::convert::Infallible;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use serde::Serialize;

/// The `?page=` query parameter, defaulting to 1.
///
/// ```
/// # use renox::prelude::*;
/// # #[derive(Model, serde::Serialize, Default)] struct Produk { id: i64 }
/// async fn index(State(db): State<Db>, Page(page): Page) -> Result<View> {
///     let produk = Produk::query().latest().paginate(&db, page, 20).await?;
///     Ok(view("produk/index.html", context! { produk }))
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page(pub u32);

impl<S: Send + Sync> FromRequestParts<S> for Page {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Infallible> {
        let page = parts
            .uri
            .query()
            .and_then(|query| {
                form_urlencoded::parse(query.as_bytes())
                    .find(|(key, _)| key == "page")
                    .and_then(|(_, value)| value.parse().ok())
            })
            .unwrap_or(1u32)
            .max(1);
        Ok(Self(page))
    }
}

/// One page of results. Render its links with the built-in macro:
///
/// ```jinja
/// {% from "renox/pagination.html" import pagination %}
/// {{ pagination(produk) }}
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct Paginated<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub per_page: u32,
    pub total: u64,
    pub last_page: u32,
    /// Position of the first and last item on this page (1-based), 0 when empty.
    pub from: u64,
    pub to: u64,
    pub has_prev: bool,
    pub has_next: bool,
    /// Page numbers to link to; `None` marks a gap ("…").
    pub pages: Vec<Option<u32>>,
}

impl<T> Paginated<T> {
    pub fn new(items: Vec<T>, page: u32, per_page: u32, total: u64) -> Self {
        let last_page = total.div_ceil(u64::from(per_page)).max(1) as u32;
        let from = if items.is_empty() {
            0
        } else {
            u64::from(page - 1) * u64::from(per_page) + 1
        };
        let to = if items.is_empty() {
            0
        } else {
            from + items.len() as u64 - 1
        };
        Self {
            items,
            page,
            per_page,
            total,
            last_page,
            from,
            to,
            has_prev: page > 1,
            has_next: page < last_page,
            pages: window(page, last_page),
        }
    }

    pub fn map<U>(self, f: impl FnMut(T) -> U) -> Paginated<U> {
        Paginated {
            items: self.items.into_iter().map(f).collect(),
            page: self.page,
            per_page: self.per_page,
            total: self.total,
            last_page: self.last_page,
            from: self.from,
            to: self.to,
            has_prev: self.has_prev,
            has_next: self.has_next,
            pages: self.pages,
        }
    }
}

/// First, last, and two pages either side of the current one.
fn window(page: u32, last: u32) -> Vec<Option<u32>> {
    let mut pages = Vec::new();
    let mut previous = 0;
    for n in 1..=last {
        if n == 1 || n == last || n.abs_diff(page) <= 2 {
            if n > previous + 1 {
                pages.push(None);
            }
            pages.push(Some(n));
            previous = n;
        }
    }
    pages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_pages_and_positions() {
        let p = Paginated::new(vec![11, 12, 13], 2, 10, 23);
        assert_eq!((p.last_page, p.from, p.to), (3, 11, 13));
        assert!(p.has_prev && p.has_next);

        let empty = Paginated::<i32>::new(vec![], 1, 10, 0);
        assert_eq!((empty.last_page, empty.from, empty.to), (1, 0, 0));
        assert!(!empty.has_prev && !empty.has_next);
    }

    #[test]
    fn windows_page_links() {
        assert_eq!(window(1, 3), vec![Some(1), Some(2), Some(3)]);
        assert_eq!(
            window(10, 20),
            vec![
                Some(1),
                None,
                Some(8),
                Some(9),
                Some(10),
                Some(11),
                Some(12),
                None,
                Some(20)
            ]
        );
        assert_eq!(
            window(2, 9),
            vec![Some(1), Some(2), Some(3), Some(4), None, Some(9)]
        );
    }
}

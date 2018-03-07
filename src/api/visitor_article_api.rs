use sapper::{Request, Response, Result as SapperResult, SapperModule, SapperRouter};
use sapper::header::{ContentType, Location};
use sapper::status;
use sapper_std::{set_cookie, JsonParams, QueryParams};
use serde_json;
use uuid::Uuid;

use super::super::{LoginUser, NewArticleStats, Postgresql, RUser, Redis, RegisteredUser,
                   UserNotify};
use super::super::{inner_get_github_nickname_and_address, inner_get_github_token};
use super::super::models::{Article, CommentWithNickName};
use super::super::page_size;
use super::super::{get_real_ip_from_req, get_ruser_from_session, get_user_agent_from_req};

pub struct VisitorArticleApi;

impl VisitorArticleApi {
    fn articles_paging(req: &mut Request) -> SapperResult<Response> {
        let pg_pool = req.ext().get::<Postgresql>().unwrap().get().unwrap();

        let mut response = Response::new();
        response.headers_mut().set(ContentType::json());

        let query_params = get_query_params!(req);
        let section_id: Uuid = match t_param!(query_params, "id").clone().parse() {
            Ok(i) => i,
            Err(err) => return res_400!(format!("UUID invalid: {}", err)),
        };

        let page: i64 = match t_param_default!(query_params, "page", "1").parse() {
            Ok(i) => i,
            Err(err) => return res_400!(format!("missing page param: {}", err)),
        };

        match Article::query_articles_with_section_id_paging(
            &pg_pool,
            section_id,
            page,
            page_size(),
        ) {
            Ok(arts_with_count) => {
                let res = json!({
                "status": true,
                "articles": arts_with_count.articles,
                "total": arts_with_count.total,
                "max_page": arts_with_count.max_page,
            });

                response.write_body(serde_json::to_string(&res).unwrap());
            }
            Err(err) => {
                let res = json!({
                "status": false,
                "error": err,
            });

                response.write_body(serde_json::to_string(&res).unwrap());
            }
        };
        Ok(response)
    }

    fn article_query(req: &mut Request) -> SapperResult<Response> {
        let pg_pool = req.ext().get::<Postgresql>().unwrap().get().unwrap();
        let redis_pool = req.ext().get::<Redis>().unwrap();
        let mut response = Response::new();
        response.headers_mut().set(ContentType::json());

        let query_params = get_query_params!(req);
        let article_id: Uuid = match t_param!(query_params, "id").clone().parse() {
            Ok(i) => i,
            Err(err) => return res_400!(format!("UUID invalid: {}", err)),
        };

        match Article::query_article_md(&pg_pool, article_id) {
            Ok(data) => {
                let session_user = get_ruser_from_session(req);
                // create article view record
                let article_stats = NewArticleStats {
                    article_id: article_id,
                    ruser_id: session_user.clone().map(|user| user.id),
                    user_agent: get_user_agent_from_req(req),
                    visitor_ip: get_real_ip_from_req(req),
                };
                article_stats.insert(&pg_pool).unwrap();

                // remove user's notify about this article
                if let Some(user) = session_user.clone() {
                    UserNotify::remove_notifys_for_article(user.id, article_id, &redis_pool);
                }

                let res = json!({
                    "status": true,
                    "data": data,
                });

                response.write_body(serde_json::to_string(&res).unwrap());
            }
            Err(err) => {
                let res = json!({
                "status": false,
                "error": err,
            });

                response.write_body(serde_json::to_string(&res).unwrap());
            }
        };
        Ok(response)
    }

    fn blogs_paging(req: &mut Request) -> SapperResult<Response> {
        let pg_pool = req.ext().get::<Postgresql>().unwrap().get().unwrap();

        let mut response = Response::new();
        response.headers_mut().set(ContentType::json());

        let query_params = get_query_params!(req);

        let page: i64 = match t_param_default!(query_params, "page", "1").parse() {
            Ok(i) => i,
            Err(err) => return res_400!(format!("missing page param: {}", err)),
        };

        match Article::query_blogs_paging(&pg_pool, 1, page, page_size()) {
            Ok(arts_with_count) => {
                let res = json!({
                    "status": true,
                    "articles": arts_with_count.articles,
                    "total": arts_with_count.total,
                    "max_page": arts_with_count.max_page,
                });

                response.write_body(serde_json::to_string(&res).unwrap());
            }
            Err(err) => {
                let res = json!({
                    "status": false,
                    "error": err,
                });

                response.write_body(serde_json::to_string(&res).unwrap());
            }
        };
        Ok(response)
    }

}

impl SapperModule for VisitorArticleApi {
    fn router(&self, router: &mut SapperRouter) -> SapperResult<()> {
        router.get("/article/paging", VisitorArticleApi::articles_paging);
        router.get("/article/get", VisitorArticleApi::article_query);
        router.get("/blogs/paging", VisitorArticleApi::blogs_paging);

        Ok(())
    }
}

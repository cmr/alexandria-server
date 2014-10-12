extern crate alexandria;
extern crate iron;
extern crate persistent;
extern crate postgres;
extern crate router;
extern crate serialize;

use std::io::net::ip::Ipv4Addr;
use serialize::json;

use iron::{ChainBuilder, Chain, Iron, status, Request, Response, IronResult, Plugin, typemap};
use persistent::{Write};
use postgres::{PostgresConnection, NoSsl};
use router::{Router, Params};

struct DBConn;
impl typemap::Assoc<PostgresConnection> for DBConn { }

#[deriving(Encodable)]
struct APIResult<T> {
    success: bool,
    data: T
}

fn good<'a, T: serialize::Encodable<json::Encoder<'a>, std::io::IoError>>(val: &T) -> Response {
    use iron::headers::content_type::MediaType;

    let json = json::encode(&APIResult { success: true, data: val });
    let mut res = Response::with(status::Ok, json);
    res.headers.content_type =
        Some(MediaType::new("application".to_string(), "json".to_string(), Vec::new()));
    res
}

fn book_from_row(row: postgres::PostgresRow) -> alexandria::Book {
    alexandria::Book {
        name: row.get("name"),
        description: row.get("description"),
        isbn: row.get("isbn"),
        cover_image: row.get("cover_image"),
        available: row.get("available"),
        quantity: row.get("quantity"),
        active_date: row.get("active_date"),
        permission: alexandria::enum_from_id(row.get("permission")).unwrap()
    }
}


fn book_query(req: &mut Request) -> IronResult<Response> {
    // do something with the query string...
    Ok(Response::status(status::NotImplemented))
}

fn get_book_by_isbn(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    Ok(match req.extensions.find::<Router, Params>().unwrap().find("isbn") {
        Some(isbn) => {
            let conn = conn.lock();
            let stmt = conn.prepare("SELECT * FROM books WHERE isbn = $1").unwrap();
            for row in stmt.query(&[&String::from_str(isbn)]).unwrap() {
                let book = book_from_row(row);
                return Ok(good(&book))
            }

            Response::status(status::NotFound)
        },
        None => Response::status(status::BadRequest)
    })
}


fn main() {
    let conn = PostgresConnection::connect("postgres://alexandria@localhost", &NoSsl).unwrap();

    let mut router = Router::new();

    router.get("/book/:isbn", get_book_by_isbn);
    router.get("/book", book_query);

    let mut chain = ChainBuilder::new(router);
    chain.link_before(Write::<DBConn, PostgresConnection>::one(conn));

    Iron::new(chain).listen(Ipv4Addr(127, 0, 0, 1), 13699);
}

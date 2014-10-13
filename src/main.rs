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

//get the json form a good request
fn good<'a, T: serialize::Encodable<json::Encoder<'a>, std::io::IoError>>(val: &T) -> Response {
    use iron::headers::content_type::MediaType;

    let json = json::encode(&APIResult { success: true, data: val });
    let mut res = Response::with(status::Ok, json);
    res.headers.content_type =
        Some(MediaType::new("application".to_string(), "json".to_string(), Vec::new()));
    res
}

//Is book from the postgres
fn book(row: postgres::PostgresRow) -> alexandria::Book {
    alexandria::Book {
        name: row.get("name"),								//name of book
        description: row.get("description"),  //description of book
        isbn: row.get("isbn"),								//isbn of book
        cover_image: row.get("cover_image"),  //cover image of book
        available: row.get("available"),			//checkout state
        quantity: row.get("quantity"),				//quantity of book in library
        active_date: row.get("active_date"),	//time of most recent operation
        permission: alexandria::enum_from_id(row.get("permission")).unwrap() //permission status
    }
}

//list of books from request
fn book_query(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    Ok(match req.extensions.find::<Router, Params>().unwrap().find("book") {
        Some(book) => {
            let conn = conn.lock();
            let stmt = conn.prepare("SELECT * FROM books WHERE book = $1").unwrap();
            let mut books = Vec::new();
            for row in stmt.query(&[&String::from_str(book)]).unwrap() {
                let books = books.push(good(book_from_row(row)));
            }
            		return Ok(&books)
            Response::status(status::NotFound)
        },
        None => Response::status(status::BadRequest)
    })
}

//a book from request
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
	//parameters for connection to database
	let params = PostgresConnectParams {
  	target: TargetUnix(some_crazy_path),//target server
  	port: None,													//target port
  	user: Some(PostgresUserInfo {				//user to login as
    	user: "postgres".to_string(),
    	password: None
  	}),
  	database: None,											//database to connect to
  	options: vec![],										//runtime parameters
	};
	//make sure params is correct
	//into_connect_params(params);
	//connection function
  let conn = PostgresConnection::connect("postgres://alexandria@localhost", &NoSsl).unwrap();

  let mut router = Router::new();

  //get book from the isbn
  router.get("/book/:isbn", get_book_by_isbn);
  //get list of books
  router.get("/book", book_query);
  //add book from isbn
  router.post("/book/:isbn", add_book_by)


  //manages the request through IRON Middleware web framework
  let mut chain = ChainBuilder::new(router);
  chain.link_before(Write::<DBConn, PostgresConnection>::one(conn));

  //kick off teh server process
  Iron::new(chain).listen(Ipv4Addr(127, 0, 0, 1), 13699);
}

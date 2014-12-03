#![feature(phase)]
extern crate alexandria;
extern crate iron;
extern crate persistent;
extern crate postgres;
extern crate router;
extern crate serialize;
extern crate bodyparser;
extern crate logger;
extern crate hyper;
extern crate url;
extern crate time;
extern crate mount;
extern crate "static" as static_file;
#[phase(plugin)] extern crate lazy_static;

use std::io::net::ip::Ipv4Addr;
use serialize::json;

use iron::{ChainBuilder, Chain, Iron, status, Request, Response, IronResult, Plugin, Set, typemap};
use persistent::{Write};
use postgres::{Connection, SslMode};
use router::{Router, Params};
use bodyparser::BodyParser;
use logger::Logger;
use mount::Mount;
use static_file::Static;

lazy_static! {
    static ref APIKEY: String = {
        std::os::getenv("GOOGLE_APIKEY").expect("Invalid API key!")
    };
}

struct DBConn;
impl typemap::Assoc<Connection> for DBConn { }

#[deriving(Encodable)]
struct APIResult<T> {
    success: bool,
    data: T
}

fn resp<B: iron::response::modifiers::Bodyable>(st: iron::status::Status, b: B) -> Response {
    Response::new()
        .set(iron::response::modifiers::Status(st))
        .set(iron::response::modifiers::Body(b))
}

fn stat(st: iron::status::Status) -> Response {
    Response::new()
        .set(iron::response::modifiers::Status(st))
}


//get the json form a good request
fn good<'a, T: serialize::Encodable<json::Encoder<'a>, std::io::IoError>>(val: &T) -> Response {
    use iron::headers::content_type::MediaType;

    let json = json::encode(&APIResult { success: true, data: val });
    let mut res = resp(status::Ok, json);
    res.headers.content_type =
        Some(MediaType::new("application".to_string(), "json".to_string(), Vec::new()));
    res
}

fn verify_isbn(_isbn: &str) -> bool {
    true
}

fn fetch_isbn(isbn: &str) -> Option<alexandria::Book> {
    use hyper::client::Request;
    use url::Url;

    let url = Url::parse(format!("https://www.googleapis.com/books/v1/volumes?q=isbn:{}&key={}",
                                 isbn, APIKEY.deref()).as_slice()).unwrap();
    let data = Request::get(url).unwrap();
    let str = data.start().unwrap().send().unwrap().read_to_end().unwrap();
    let str = std::str::from_utf8(str.as_slice()).unwrap();
    println!("Got string: {}", str);
    let resp = match json::from_str(str) {
        Ok(json) => json,
        Err(e) => {
            println!("Error decoding JSON: {}", e);
            return None;
        },
    };

    // now for some pain!
    let avail = resp.find("totalItems").expect("no totalItems").as_u64().expect("totalItems not a u64");
    if avail == 0 {
        return None;
    } else if avail > 1 {
        println!("Google API returned multiple items for a single ISBN {}? Please report a bug!", isbn);
    }
    let item = &resp.find("items").expect("no items").as_list().expect("items not a list")[0];
    // time to pull out all the data we care about

    Some(alexandria::Book {
    name: item.find_path(&["volumeInfo", "title"]).unwrap().as_string().unwrap().to_string(),
    description: item.find_path(&["volumeInfo", "description"]).unwrap().as_string().unwrap().to_string(),
    isbn: isbn.to_string(),
    cover_image: match item.find_path(&["volumeInfo", "imageLinks", "thumbnail"]) {
        Some(url) => url.as_string().unwrap().to_string(),
        None => String::new()
    },
    available: 0,
    quantity: 0,
    active_date: time::Timespec::new(0, 0),
    permission: alexandria::DontLeaveLibrary
    })
}

//Is book from the postgress
fn book_from_row(row: postgres::Row) -> alexandria::Book {
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

//Is student form postgress
fn student_from_row(row: postgres::Row) -> alexandria::User {
	alexandria::User{
		name: row.get("name"),			//name of student
		email: row.get("email"),		//email of student
		student_id: row.get("id"),	//id of student
		permission: alexandria::enum_from_id(row.get("permission")).unwrap() //permissions status
	}
}

//Is history form postgress
fn history_from_row(row: postgres::Row) -> alexandria::History {
    alexandria::History{
        isbn: row.get("isbn"),
        book: row.get("book"),
        available: row.get("available"),
        quantity: row.get("quantity"),
        student_id: row.get("student_id"),        //student_id of history
        date: row.get("date"),  //date of history
        action: alexandria::enum_from_id(row.get("action")).unwrap() //checkstatus
    }
}

//list of books from request
fn get_books(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, Connection>>().unwrap();
    let conn = conn.lock();
    let stmt = conn.prepare("SELECT * FROM books").unwrap();
    return Ok(good(&stmt.query(&[]).unwrap().map(|row| book_from_row(row)).collect::<Vec<_>>()))
}

//a book from request
fn get_book_by_search(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, Connection>>().unwrap();
    Ok(match req.extensions.get::<Router, Params>().unwrap().find("isbn") {
        Some(isbn) => {
            let conn = conn.lock();
            let stmt = conn.prepare("SELECT * FROM books WHERE isbn=$1 OR name=$1").unwrap();
            for row in stmt.query(&[&String::from_str(isbn)]).unwrap() {
                let book = book_from_row(row);
                return Ok(good(&book))
            }

            stat(status::NotFound)
        },
        None => stat(status::BadRequest)
    })
}

//update book from request
fn update_book_by_isbn(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, Connection>>().unwrap();
    let isbn;
    {
        match req.extensions.get::<Router, Params>().unwrap().find("isbn") {
            Some(isbn_) => {
                isbn = isbn_.to_string();
            },
            None => return Ok(stat(status::BadRequest))
        }
    }
    let conn = conn.lock();
    let stmt = conn.prepare("UPDATE books SET name=$1,description=$2,isbn=$3,cover_image=$4,available=$5,quantity=$6,active_date=$7,permission=$8) WHERE isbn=$9").unwrap();
    let parsed = req.get::<BodyParser<alexandria::Book>>().unwrap();
    match stmt.execute(&[&parsed.name,&parsed.description,&parsed.isbn,
       &parsed.cover_image,&parsed.available,&parsed.quantity,&parsed.active_date,
       &(parsed.permission as i16),&isbn]) {
        Ok(num) => println!("Update Book! {}", num),
        Err(err) => println!("Error executing update_book_by_isbn: {}", err)
    }
    Ok(stat(status::NotFound))
}

fn add_book(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, Connection>>().unwrap();
    let isbn;
    {
        match req.extensions.get::<Router, Params>().unwrap().find("isbn") {
            Some(isbn_) => {
                isbn = isbn_.to_string();
            },
            None => return Ok(stat(status::BadRequest))
        }
    }

    if verify_isbn(isbn.as_slice()) == false {
        return Ok(stat(status::BadRequest))
    }

    let book = fetch_isbn(isbn.as_slice()).unwrap();
    let conn = conn.lock();
    let stmt = conn.prepare("INSERT INTO books (name,description,isbn,cover_image,available,quantity,active_date,permission) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)").unwrap();
    let parsed = book;
    match stmt.execute(&[&parsed.name,&parsed.description,&parsed.isbn,
       &parsed.cover_image,&parsed.available,&parsed.quantity,&parsed.active_date,
       &(parsed.permission as i16)]) {
        Ok(num) => println!("Added Book! {}", num),
        Err(err) => {
            println!("Error executing add_book: {}", err);
            return Ok(stat(status::InternalServerError));
        }
    }
    Ok(stat(status::Ok))
}

//delete book from request
fn delete_book_by_isbn(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, Connection>>().unwrap();
    let isbn;
    {
        match req.extensions.get::<Router, Params>().unwrap().find("isbn") {
            Some(isbn_) => {
                isbn = isbn_.to_string();
            },
            None => return Ok(stat(status::BadRequest))
        }
    }
    let conn = conn.lock();
    let stmt = conn.prepare("DELETE FROM books WHERE isbn=$1").unwrap();
    match stmt.execute(&[&isbn]) {
        Ok(num) => println!("Deleted Book! {}", num),
        Err(err) => {
            println!("Error executing delete_book_by_isbn: {}", err);
            return Ok(stat(status::InternalServerError));
        }
    }
    Ok(stat(status::NotFound))
}

//list of students from request
fn get_students(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, Connection>>().unwrap();
    let conn = conn.lock();
    let stmt = conn.prepare("SELECT * FROM users").unwrap();
    let mut users = Vec::new();
    for row in stmt.query(&[]).unwrap() {
        users.push(student_from_row(row));
    }

    return Ok(good(&users))
}

//students from request
fn get_student_by_name(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, Connection>>().unwrap();
    Ok(match req.extensions.get::<Router, Params>().unwrap().find("user"){
        Some(user) => {
            let conn = conn.lock();
            let stmt = conn.prepare("SELECT * FROM users WHERE student_id=$1").unwrap();
            for row in stmt.query(&[&String::from_str(user)]).unwrap() {
                let student = book_from_row(row);
                return Ok(good(&student))
            }

            stat(status::NotFound)
        },
        None => stat(status::BadRequest)
    })
}

//update student from request
fn update_student_by_id(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, Connection>>().unwrap();
    let student_id;
    {
        match req.extensions.get::<Router, Params>().unwrap().find("id") {
            Some(student_id_) => {
                student_id = student_id_.to_string();
            },
            None => return Ok(stat(status::BadRequest))
        }
    }
    let conn = conn.lock();
    let stmt = conn.prepare("UPDATE users SET name=$1,email=$2,permission=$4 WHERE student_id=$1").unwrap();
    let parsed = req.get::<BodyParser<alexandria::User>>().unwrap();
    match stmt.execute(&[&student_id,&parsed.name,&parsed.email,&(parsed.permission as i16)]) {
        Ok(num) => println!("Update Student! {}", num),
        Err(err) => {
            println!("Error executing update_student_by_name: {}", err);
            return Ok(stat(status::InternalServerError));
        }
    }
    Ok(stat(status::NotFound))
}

//add student from request
fn add_student(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, Connection>>().unwrap();
    let conn = conn.lock();
    // TODO: Handle user already exists!
    let stmt = conn.prepare("INSERT INTO users (name,email,student_id,permission) VALUES ($1,$2,$3,$4)").unwrap();
    let parsed = req.get::<BodyParser<alexandria::User>>().unwrap();
    match stmt.execute(&[&parsed.name,&parsed.email,&parsed.student_id,&(parsed.permission as i16)]) {
        Ok(num) => println!("Added Student! {}", num),
        Err(err) => {
            println!("Error executing add_student: {}", err);
            return Ok(stat(status::InternalServerError));
        }
    }
    Ok(stat(status::NotFound))
}

//delete student from request
fn delete_student_by_id(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, Connection>>().unwrap();
    let student_id;
    match req.extensions.get::<Router, Params>().unwrap().find("id") {
        Some(student_id_) => {
            student_id = student_id_.to_string();
        },
        None => return Ok(stat(status::BadRequest))
    }
    let conn = conn.lock();
    let stmt = conn.prepare("DELETE from users WHERE student_id=$1").unwrap();
    match stmt.execute(&[&student_id]) {
        Ok(num) => println!("Deleted Student! {}", num),
        Err(err) => {
            println!("Error executing delete_student_by_name: {}", err);
            return Ok(stat(status::InternalServerError));
        }
    }
    Ok(stat(status::NotFound))
}

//get checkoutstatus of a book
fn checkout(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, Connection>>().unwrap();
    let parsed = req.get::<BodyParser<alexandria::ActionRequest>>().unwrap();
    let mut book_history = Vec::new();
    let conn = conn.lock();
    let stmt1 = conn.prepare("SELECT isbn, quantity, student_id, available FROM books INNER JOIN history ON books.isdn = history.isbn WHERE books.isbn=$1").unwrap();
    for row in stmt1.query(&[&parsed.isbn]).unwrap(){
        book_history.push(history_from_row(row));

    }
    if book_history[0].available <= book_history[0].quantity {
        println!("NO Books Available");
        return Ok(Response::status(status::InternalServerError));
    }else{
        let stmt2 = conn.prepare("INSERT INTO history (student_id,book,date,action) VALUES ($1,$2,$3,$4)").unwrap();
        match stmt2.execute(&[&parsed.student_id,&parsed.isbn,&time::get_time(),&(parsed.action as i16)]){
            Ok(num) => println!("Insert Into History! {}", num),
            Err(err) => {
                println!("Error executing checkin Insert Into: {}", err);
                return Ok(stat(status::InternalServerError));
            }
        };
        let stmt3 = conn.prepare("UPDATE books SET available=$1 WHERE isbn=$2").unwrap();
        match stmt3.execute(&[&(book_history[0].available-1,&parsed.isbn)]){
            Ok(num) => println!("Update History! {}", num),
            Err(err) => {
                println!("Error executing checkin update: {}", err);
                return Ok(stat(status::InternalServerError));
            }
        };
    }
    Ok(stat(status::NotFound))
}

fn ignore_auth(req: &mut Request) -> IronResult<Response> {
    Ok(stat(status::Ok))
}

//get checkinstatus of a book
fn checkin(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, Connection>>().unwrap();
    let parsed = req.get::<BodyParser<alexandria::ActionRequest>>().unwrap();
    let conn = conn.lock();
    let mut book_history = Vec::new();
    let stmt1 = conn.prepare("SELECT isbn, quantity, student_id, available FROM books INNER JOIN history ON books.isbn = history.isbn WHERE books.isbn=$1").unwrap();
    for row in stmt1.query(&[&parsed.isbn]).unwrap(){
        book_history.push(history_from_row(row));
    }
    if book_history.len() <= 0 {
        println!("NO Books to Checkin");
        return Ok(Response::status(status::InternalServerError));
    }else{
        let stmt2 = conn.prepare("DELETE from history WHERE student_id=$1 AND isbn=$2").unwrap();
        match stmt2.execute(&[&parsed.student_id,&parsed.isbn]){
            Ok(num) => println!("Deleted History! {}", num),
            Err(err) => {
                println!("Error executing checkin delete: {}", err);
                return Ok(stat(status::InternalServerError));
            }
        };
        let stmt3 = conn.prepare("UPDATE books SET available=$1 WHERE isbn=$2").unwrap();
        match stmt3.execute(&[&(book_history[0].available+1,&parsed.isbn)]){
            Ok(num) => println!("Update History! {}", num),
            Err(err) => {
                println!("Error executing checkin update: {}", err);
                return Ok(stat(status::InternalServerError));
            }
        };
    }
    Ok(stat(status::NotFound))
}

fn main() {

  //connection function
  let conn = Connection::connect("postgres://alexandria@localhost", &SslMode::None).unwrap();

  let mut router = Router::new();

  // TODO: paginate all of these

  router.get("/auth", ignore_auth);
  //get list of books
  router.get("/book", get_books);
  //get book from the isbn
  router.get("/book/:isbn", get_book_by_search);
  //update book from isbn
  router.post("/book/:isbn", update_book_by_isbn);
  //add book from isbn
  router.put("/book/:isbn", add_book);
  //delete book from isbn
  router.delete("/book/:isbn", delete_book_by_isbn);


  //get list of students
  router.get("/student", get_students);
  //add student from name
  router.put("/student", add_student);
  //get student
  router.get("/student/:id", get_student_by_name);
  //delete students from id
  router.delete("/student/:id", delete_student_by_id);
  //update student from id
  router.post("/student/:id", update_student_by_id);


  //checkout
  router.post("/checkout", checkout);
  //checkin
  router.post("/checkin", checkin);

  //manages the request through IRON Middleware web framework

  let mut mount = Mount::new();
  mount.mount("/", Static::new(Path::new("../alexandria-web-client/jquery/index.html")));
  mount.mount("/api/v1", router);
  mount.mount("/static/", Static::new(Path::new("../alexandria-web-client/jquery/static/")));

  let mut chain = ChainBuilder::new(mount);

  let (logger_before, logger_after) = Logger::new(None);
  chain.link_before(logger_before);
  chain.link_before(Write::<DBConn, Connection>::one(conn));

  // this must be last
  chain.link_after(logger_after);
  //kick off the server process
  Iron::new(chain).listen(Ipv4Addr(0, 0, 0, 0), 13699);
}

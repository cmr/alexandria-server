extern crate alexandria;
extern crate iron;
extern crate persistent;
extern crate postgres;
extern crate router;
extern crate serialize;
extern crate bodyparser;
extern crate logger;

use std::io::net::ip::Ipv4Addr;
use serialize::json;

use iron::{ChainBuilder, Chain, Iron, status, Request, Response, IronResult, Plugin, typemap};
use persistent::{Write};
use postgres::{PostgresConnection, NoSsl};
use router::{Router, Params};
use bodyparser::BodyParser;
use logger::Logger;

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

//Is book from the postgress
fn book_from_row(row: postgres::PostgresRow) -> alexandria::Book {
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
fn student_from_row(row: postgres::PostgresRow) -> alexandria::User {
	alexandria::User{
		name: row.get("name"),			//name of student
		email: row.get("email"),		//email of student
		student_id: row.get("id"),	//id of student
		permission: alexandria::enum_from_id(row.get("permission")).unwrap() //permissions status
	}
}

//Is history form postgress
fn history_from_row(row: postgres::PostgresRow) -> alexandria::History {
    alexandria::History{
        isbn: row.get("isbn"),          //isbn of history
        student_id: row.get("student_id"),        //student_id of history
        date: row.get("date"),  //date of history
        action: alexandria::enum_from_id(row.get("action")).unwrap() //checkstatus
    }
}

//list of books from request
fn get_books(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    let conn = conn.lock();
    let stmt = conn.prepare("SELECT * FROM books").unwrap();
    return Ok(good(&stmt.query([]).unwrap().map(|row| book_from_row(row)).collect::<Vec<_>>()))
}

//a book from request
fn get_book_by_isbn(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    Ok(match req.extensions.find::<Router, Params>().unwrap().find("isbn") {
        Some(isbn) => {
            let conn = conn.lock();
            let stmt = conn.prepare("SELECT * FROM books WHERE isbn=$1").unwrap();
            for row in stmt.query(&[&String::from_str(isbn)]).unwrap() {
                let book = book_from_row(row);
                return Ok(good(&book))
            }

            Response::status(status::NotFound)
        },
        None => Response::status(status::BadRequest)
    })
}

//update book from request
fn update_book_by_isbn(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    let isbn;
    {
        match req.extensions.find::<Router, Params>().unwrap().find("isbn") {
            Some(isbn_) => {
                isbn = isbn_.to_string();
            },
            None => return Ok(Response::status(status::BadRequest))
        }
    }
    let conn = conn.lock();
    let stmt = conn.prepare("UPDATE books SET name=$1,description=$2,isbn=$3,cover_image=$4,available=$5,quantity=$6,active_date=$7,permission=$8 WHERE isbn=$9").unwrap();
    let parsed = req.get::<BodyParser<alexandria::Book>>().unwrap();
    match stmt.execute(&[&parsed.name,&parsed.description,&parsed.isbn,
       &parsed.cover_image,&parsed.available,&parsed.quantity,&parsed.active_date,
       &(parsed.permission as i16),&isbn]) {
        Ok(num) => println!("Update Book! {}", num),
        Err(err) => println!("Error executing update_book_by_isbn: {}", err)
    }
    Ok(Response::status(status::NotFound))
}

//add book from request
fn add_book(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    let conn = conn.lock();
    let stmt = conn.prepare("INSERT INTO books (name,description,isbn,cover_image,available,quantity,active_date,permission) VALUES ($1,$2,$3,$4,$5,$6,$7,$8").unwrap();
    let parsed = req.get::<BodyParser<alexandria::Book>>().unwrap();
    match stmt.execute(&[&parsed.name,&parsed.description,&parsed.isbn,
       &parsed.cover_image,&parsed.available,&parsed.quantity,&parsed.active_date,
       &(parsed.permission as i16)]) {
        Ok(num) => println!("Added Book! {}", num),
        Err(err) => {
            println!("Error executing add_book: {}", err);
            return Ok(Response::status(status::InternalServerError));
        }
    }
    Ok(Response::status(status::NotFound))
}

//delete book from request
fn delete_book_by_isbn(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    let isbn;
    {
        match req.extensions.find::<Router, Params>().unwrap().find("isbn") {
            Some(isbn_) => {
                isbn = isbn_.to_string();
            },
            None => return Ok(Response::status(status::BadRequest))
        }
    }
    let conn = conn.lock();
    let stmt = conn.prepare("DELETE FROM books WHERE isbn=$1").unwrap();
    match stmt.execute(&[&isbn]) {
        Ok(num) => println!("Deleted Book! {}", num),
        Err(err) => {
            println!("Error executing delete_book_by_isbn: {}", err);
            return Ok(Response::status(status::InternalServerError));
        }
    }
    Ok(Response::status(status::NotFound))
}

//list of students from request
fn get_students(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    let conn = conn.lock();
    let stmt = conn.prepare("SELECT * FROM users").unwrap();
    let mut users = Vec::new();
    for row in stmt.query([]).unwrap() {
        users.push(student_from_row(row));
    }

    return Ok(good(&users))
}

//students from request
fn get_student_by_name(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    Ok(match req.extensions.find::<Router, Params>().unwrap().find("user"){
        Some(user) => {
            let conn = conn.lock();
            let stmt = conn.prepare("SELECT * FROM users WHERE student_id=$1").unwrap();
            for row in stmt.query(&[&String::from_str(user)]).unwrap() {
                let student = book_from_row(row);
                return Ok(good(&student))
            }

            Response::status(status::NotFound)
        },
        None => Response::status(status::BadRequest)
    })
}

//update student from request
fn update_student_by_id(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    let student_id;
    {
        match req.extensions.find::<Router, Params>().unwrap().find("id") {
            Some(student_id_) => {
                student_id = student_id_.to_string();
            },
            None => return Ok(Response::status(status::BadRequest))
        }
    }
    let conn = conn.lock();
    let stmt = conn.prepare("UPDATE users SET name=$1,email=$2,permission=$4 WHERE student_id=$1").unwrap();
    let parsed = req.get::<BodyParser<alexandria::User>>().unwrap();
    match stmt.execute(&[&student_id,&parsed.name,&parsed.email,&(parsed.permission as i16)]) {
        Ok(num) => println!("Update Student! {}", num),
        Err(err) => {
            println!("Error executing update_student_by_name: {}", err);
            return Ok(Response::status(status::InternalServerError));
        }
    }
    Ok(Response::status(status::NotFound))
}

//add student from request
fn add_student(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    let conn = conn.lock();
    // TODO: Handle user already exists!
    let stmt = conn.prepare("INSERT INTO users (name,email,student_id,permission) VALUES ($1,$2,$3,$4)").unwrap();
    let parsed = req.get::<BodyParser<alexandria::User>>().unwrap();
    match stmt.execute(&[&parsed.name,&parsed.email,&parsed.student_id,&(parsed.permission as i16)]) {
        Ok(num) => println!("Added Student! {}", num),
        Err(err) => {
            println!("Error executing add_student: {}", err);
            return Ok(Response::status(status::InternalServerError));
        }
    }
    Ok(Response::status(status::NotFound))
}

//delete student from request
fn delete_student_by_id(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    let student_id;
    match req.extensions.find::<Router, Params>().unwrap().find("id") {
        Some(student_id_) => {
            student_id = student_id_.to_string();
        },
        None => return Ok(Response::status(status::BadRequest))
    }
    let conn = conn.lock();
    let stmt = conn.prepare("DELETE from users WHERE student_id=$1").unwrap();
    match stmt.execute(&[&student_id]) {
        Ok(num) => println!("Deleted Student! {}", num),
        Err(err) => {
            println!("Error executing delete_student_by_name: {}", err);
            return Ok(Response::status(status::InternalServerError));
        }
    }
    Ok(Response::status(status::NotFound))
}

//get checkoutstatus of a book
fn checkout(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    let parsed = req.get::<BodyParser<alexandria::ActionRequest>>().unwrap();
    let action;
    let conn = conn.lock();
    let stmt1 = conn.prepare("SELECT * FROM history WHERE isbn=$1 AND student_id=$2 AND action=$3 ORDER BY date DESC").unwrap();
    action = history_from_row(stmt1.query(&[&parsed.isbn,&parsed.student_id,&(parsed.action as i16)]).unwrap().next().unwrap());
    if (parsed.isbn == action.isbn) && ((parsed.action as i16) == (action.action as i16)) && (parsed.student_id == action.student_id) {
        let stmt2 = conn.prepare("SELECT * FROM books WHERE isbn=$1").unwrap();
        for row in stmt2.query(&[&parsed.isbn]).unwrap() {
            let book = book_from_row(row);
            if (book.available > 0) && (book.quantity >= book.available) && (book.isbn == parsed.isbn) {
                let stmt3 = conn.prepare("UPDATE books SET available=$1 WHERE isbn=$2").unwrap();
                match stmt3.execute(&[&(book.available-1)]) {
                    Ok(num) => println!("Update Checkout! {}", num),
                    Err(err) => {
                        println!("Error executing checkout: {}", err);
                        return Ok(Response::status(status::InternalServerError));
                    }
                }
            }
        }
    }
    Ok(Response::status(status::NotFound))
}

//get checkinstatus of a book
fn checkin(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    let parsed = req.get::<BodyParser<alexandria::ActionRequest>>().unwrap();
    let action;
    let conn = conn.lock();
    let stmt1 = conn.prepare("SELECT * FROM history WHERE isbn=$1 AND student_id=$2 AND action=$3 ORDER BY date DESC").unwrap();
    action = history_from_row(stmt1.query(&[&parsed.isbn,&parsed.student_id,&(parsed.action as i16)]).unwrap().next().unwrap());
    if (parsed.isbn == action.isbn) && ((parsed.action as i16) == (action.action as i16)) && (parsed.student_id == action.student_id) {
        let stmt2 = conn.prepare("SELECT * FROM books WHERE isbn=$1").unwrap();
        for row in stmt2.query(&[&parsed.isbn]).unwrap() {
            let book = book_from_row(row);
            if (book.available >= 0) && (book.quantity > book.available) && (book.isbn == parsed.isbn) {
                let stmt3 = conn.prepare("UPDATE books SET available=$1 WHERE isbn=$2").unwrap();
                match stmt3.execute(&[&(book.available-1)]) {
                    Ok(num) => println!("Update Checkout! {}", num),
                    Err(err) => {
                        println!("Error executing checkout: {}", err);
                        return Ok(Response::status(status::InternalServerError));
                    }
                }
            }
        }
    }
    Ok(Response::status(status::NotFound))
}
/*

//history
fn history(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    let student_id;
    match req.extensions.find::<Router, Params>().unwrap().find("id") {
        Some(student_id_) => {
            student_id = student_id_.to_string();
        },
        None => return Ok(Response::status(status::BadRequest))
    }
    let conn = conn.lock();
    let stmt = conn.prepare("DELETE from users WHERE student_id=$1").unwrap();
    match stmt.execute(&[&student_id]) {
        Ok(num) => println!("Deleted Student! {}", num),
        Err(err) => {
            println!("Error executing delete_student_by_name: {}", err);
            return Ok(Response::status(status::InternalServerError));
        }
    }
    Ok(Response::status(status::NotFound))
}
*/
fn main() {
	/*
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
	};*/

	//make sure params is correct
	//into_connect_params(params);
	//connection function
  let conn = PostgresConnection::connect("postgres://alexandria@localhost", &NoSsl).unwrap();

  let mut router = Router::new();

  // TODO: paginate all of these

  //get list of books
  router.get("/book", get_books);
  //get book from the isbn
  router.get("/book/:isbn", get_book_by_isbn);
  //update book from isbn
  router.put("/book/:isbn", update_book_by_isbn);
  //add book from isbn
  router.post("/book", add_book);
  //delete book from isbn
  router.delete("/book/:isbn", delete_book_by_isbn);


  //get list of students
  router.get("/student", get_students);
  //add student from name
  router.post("/student", add_student);
  //get student
  router.get("/student/:id", get_student_by_name);
  //delete students from id
  router.delete("/student/:id", delete_student_by_id);
  //update student from id
  router.put("/student/:id", update_student_by_id);


  //checkout
  router.get("/checkout", checkout);
  //checkin
  router.get("/checkin", checkin);

  //history
  //router.get("/history", history);

  let (logger_before, logger_after) = Logger::new(None);
  //manages the request through IRON Middleware web framework
  let mut chain = ChainBuilder::new(router);
  chain.link_before(logger_before);
  chain.link_before(Write::<DBConn, PostgresConnection>::one(conn));

  // this must be last
  chain.link_after(logger_after);
  //kick off the server process
  Iron::new(chain).listen(Ipv4Addr(127, 0, 0, 1), 13699);
}

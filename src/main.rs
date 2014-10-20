extern crate alexandria;
extern crate iron;
extern crate persistent;
extern crate postgres;
extern crate router;
extern crate serialize;
extern crate bodyparser;

use std::io::net::ip::Ipv4Addr;
use serialize::json;

use iron::{ChainBuilder, Chain, Iron, status, Request, Response, IronResult, Plugin, typemap};
use persistent::{Write};
use postgres::{PostgresConnection, NoSsl};
use router::{Router, Params};
use bodyparser::BodyParser;

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

//Is student form postgres
fn student_from_row(row: postgres::PostgresRow) -> alexandria::User {
	alexandria::User{
		name: row.get("name"),			//name of student
		email: row.get("email"),		//email of student
		student_id: row.get("id"),	//id of student
		permission: alexandria::enum_from_id(row.get("permission")).unwrap() //permissions status
	}
}

//list of books from request
fn get_books(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    Ok(match req.extensions.find::<Router, Params>().unwrap().find("book") {
        Some(book) => {
            let conn = conn.lock();
            let stmt = conn.prepare("SELECT * FROM books").unwrap();
            let mut books = Vec::new();
            for row in stmt.query([]).unwrap() {
                let books = books.push(book_from_row(row));
            }
            return Ok(good(&books))

            //Response::status(status::NotFound)
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
            let stmt = conn.prepare("SELECT * FROM books WHERE isbn=$2").unwrap();
            for row in stmt.query([&String::from_str(isbn)]).unwrap() {
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
    Ok(match req.extensions.find::<Router, Params>().unwrap().find("isbn") {
        Some(isbn) => {
            let conn = conn.lock();
            let stmt = conn.prepare("UPDATE books SET name=$1,description=$2,isbn=$3,cover_image=$4,available=$5,quantity=$6,active_date=$7,permission=$8 WHERE isbn=$8").unwrap();
            req.get::<BodyParser>();
            match stmt.execute(&[&String::from_str(name),&String::from_str(description),&cover_image,&available,&quantity,&active_date,&permission,&isbn]) {
    					Ok(num) => println!("Update Book! {}", num),
    					Err(err) => println!("Error executing update_book_by_isbn: {}", err)
						}
            Response::status(status::NotFound)
        },
        None => Response::status(status::BadRequest)
    })
}

//add book from request
fn add_book_by_isbn(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    Ok(match req.extensions.find::<Router, Params>().unwrap().find("book") {
        Some(book) => {
            let conn = conn.lock();
            let stmt = conn.prepare("INSERT INTO books (name,description,isbn,cover_image,available,quantity,active_date,permission) VALUES ($1,$2,$3,$4,$5,$6,$7,$8").unwrap();
            
            match stmt.execute(&[&String::from_str(book.name),&String::from_str(book.description),&book.cover_image,&book.available,&book.quantity,&book.active_date,&book.permission]) {
    					Ok(num) => println!("Added Book! {}", num),
    					Err(err) => println!("Error executing add_book_by_isbn: {}", err)
						}
            Response::status(status::NotFound)
        },
        None => Response::status(status::BadRequest)
    })
}

//delete book from request
fn delete_book_by_isbn(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    Ok(match req.extensions.find::<Router, Params>().unwrap().find("Book") {
        Some(Book) => {
            let conn = conn.lock();
            let stmt = conn.prepare("DELETE FROM books WHERE isbn=$1 OR name=$2").unwrap();
            
            match stmt.execute(&[&String::from_str(Book.isbn),&String::from_str(Book.name)]) {
    					Ok(num) => println!("Deleted Book! {}", num),
    					Err(err) => println!("Error executing delete_book_by_isbn: {}", err)
						}
            Response::status(status::NotFound)
        },
        None => Response::status(status::BadRequest)
    })
}

//list of students from request
fn get_students(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    Ok(match req.extensions.find::<Router, Params>().unwrap().find("User") {
        Some(User) => {
            let conn = conn.lock();
            let stmt = conn.prepare("SELECT * FROM users").unwrap();
            let mut users = Vec::new();
            for row in stmt.query([]).unwrap() {
                users.push(student_from_row(row));
            }
            
            return Ok(good(&users))
            //Response::status(status::NotFound)
        },
        None => Response::status(status::BadRequest)
    })
}

//students from request
fn get_student_by_name(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    Ok(match req.extensions.find::<Router, Params>().unwrap().find("User"){
        Some(User) => {
            let conn = conn.lock();
            let stmt = conn.prepare("SELECT * FROM users WHERE name=$1 OR email=$2 OR student_id=$3").unwrap();
            
            for row in stmt.query(&[&String::from_str(User.name),&String::from_str(User.email),&String::from_str(User.student_id)]).unwrap() {
                let student = book_from_row(row);
                return Ok(good(&student))
            }

            Response::status(status::NotFound)
        },
        None => Response::status(status::BadRequest)
    })
}


//update student from request
fn update_student_by_name(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    Ok(match req.extensions.find::<Router, Params>().unwrap().find("User") {
        Some(User) => {
            let conn = conn.lock();
            let stmt = conn.prepare("UPDATE users SET name=$1,email=$2,student_id=$3,permission=$4 WHERE name=$1 OR email=$2 OR student_id=$3").unwrap();
            
            match stmt.execute(&[&String::from_str(User.name),&String::from_str(User.email),&String::from_str(User.student_id)]) {
    					Ok(num) => println!("Update Student! {}", num),
    					Err(err) => println!("Error executing update_student_by_name: {}", err)
						}
            Response::status(status::NotFound)
        },
        None => Response::status(status::BadRequest)
    })
}

//add student from request
fn add_student_by_name(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    Ok(match req.extensions.find::<Router, Params>().unwrap().find("User") {
        Some(User) => {
            let conn = conn.lock();
            let stmt = conn.prepare("INSERT INTO users (name,email,student_id,permission) WHERE ($1,$2,$3,$4)").unwrap();
            
            match stmt.execute(&[&String::from_str(User.name),&String::from_str(User.email),&String::from_str(User.student_id)]) {
    					Ok(num) => println!("Added Student! {}", num),
    					Err(err) => println!("Error executing add_student_by_name: {}", err)
						}
            Response::status(status::NotFound)
        },
        None => Response::status(status::BadRequest)
    })
}

//delete student from request
fn delete_student_by_name(req: &mut Request) -> IronResult<Response> {
    let conn = req.get::<Write<DBConn, PostgresConnection>>().unwrap();
    Ok(match req.extensions.find::<Router, Params>().unwrap().find("User") {
        Some(User) => {
            let conn = conn.lock();
            let stmt = conn.prepare("DELETE from users WHERE name=$1 OR email=$2 OR student_id=$3").unwrap();
            
            match stmt.execute(&[&String::from_str(User.name),&String::from_str(User.email),&String::from_str(User.student_id)]) {
    					Ok(num) => println!("Deleted Student! {}", num),
    					Err(err) => println!("Error executing delete_student_by_name: {}", err)
						}
            Response::status(status::NotFound)
        },
        None => Response::status(status::BadRequest)
    })
}

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

  //get book from the isbn
  router.get("/book/:isbn", get_book_by_isbn);
  //get list of books
  router.get("/book", get_books);
  //add book from isbn
  router.post("/book/:isbn", add_book_by_isbn);
  //delete book from isbn
  router.delete("/book", delete_book_by_isbn);
  //update book from isbn
  router.put("/book/:isbn", update_book_by_isbn);

  //get student 
  router.get("/student:name", get_student_by_name);
  //get list of students
  router.get("/student", get_students);
  //add student from name
  router.post("/student:name", add_student_by_name);
  //delete students from name
  router.delete("/student:name", delete_student_by_name);
  //update student from name
  router.put("/student:name", update_student_by_name);

  //manages the request through IRON Middleware web framework
  let mut chain = ChainBuilder::new(router);
  chain.link_before(Write::<DBConn, PostgresConnection>::one(conn));

  //kick off teh server process
  Iron::new(chain).listen(Ipv4Addr(127, 0, 0, 1), 13699);
}

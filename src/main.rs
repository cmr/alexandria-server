extern crate iron;
extern crate router;

extern crate postgres;

use std::io::net::ip::Ipv4Addr;
use iron::{Iron, status, Request, Response, IronResult};
use router::{Router, Params};

use postgres::{PostgresConnection, NoSsl};
use postgres::types::ToSql;

fn get_book(req: &mut Request) -> IronResult<Response> {
    Ok(match req.extensions.find::<Router, Params>().unwrap().find("id") {
        Some(id) => {
            Response::with(status::Ok, format!(r#"{{"success":true, "got_id": {}}}"#, id))
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
	//let cparams = into_connect_params(params);
	//connection function
  let conn = try!(PostgresConnection::connect(into_connect_params(params),&NoSsl));

  let mut router = Router::new();
  //get book id
  router.get("/book/:id", get_book);
  //delete book id
  router.delete("/book/:id", delete_book);
  //add book id
  router.post("/book/:id", add_book);
  //update book id 
  router.put("/book/:id", update_book);

  Iron::new(router).listen(Ipv4Addr(127, 0, 0, 1), 13699);
}

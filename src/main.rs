extern crate iron;
extern crate router;

use std::io::net::ip::Ipv4Addr;
use iron::{Iron, status, Request, Response, IronResult};
use router::{Router, Params};

fn get_book(req: &mut Request) -> IronResult<Response> {
    Ok(match req.extensions.find::<Router, Params>().unwrap().find("id") {
        Some(id) => {
            Response::with(status::Ok, format!(r#"{{"success":true, "got_id": {}}}"#, id))
        },
        None => Response::status(status::BadRequest)
    })
}


fn main() {
    let mut router = Router::new();
    router.get("/book/:id", get_book);

    Iron::new(router).listen(Ipv4Addr(127, 0, 0, 1), 13699);
}

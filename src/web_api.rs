use crate::context::{run_with_plugin, Context};
use crate::define_data_plugin;
use crate::error::IxaError;
use crate::external_api::{
    breakpoint, global_properties, people, population, run_ext_api, time, EmptyArgs, ExtApi,
};
use crate::{HashMap, HashMapExt};
use axum::extract::{Json, Path, State};
use axum::response::Redirect;
use axum::routing::get;
use axum::{http::StatusCode, routing::post, Router};
use rand::RngCore;
use serde_json::json;
use std::thread;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tower_http::services::{ServeDir, ServeFile};

pub type WebApiHandler =
    dyn Fn(&mut Context, serde_json::Value) -> Result<serde_json::Value, IxaError>;

fn register_api_handler<
    T: crate::external_api::ExtApi<Args = A>,
    A: serde::de::DeserializeOwned,
>(
    dc: &mut ApiData,
    name: &str,
) {
    dc.handlers.insert(
        name.to_string(),
        Box::new(
            |context, args_json| -> Result<serde_json::Value, IxaError> {
                let args: A = serde_json::from_value(args_json)?;
                let retval: T::Retval = run_ext_api::<T>(context, &args)?;
                Ok(serde_json::to_value(retval)?)
            },
        ),
    );
}

struct ApiData {
    receiver: mpsc::UnboundedReceiver<ApiRequest>,
    handlers: HashMap<String, Box<WebApiHandler>>,
}

define_data_plugin!(ApiPlugin, Option<ApiData>, None);

// Input to the API handler.
struct ApiRequest {
    cmd: String,
    arguments: serde_json::Value,
    // This channel is used to send the response.
    rx: oneshot::Sender<ApiResponse>,
}

// Output of the API handler.
struct ApiResponse {
    code: StatusCode,
    response: serde_json::Value,
}

#[derive(Clone)]
struct ApiEndpointServer {
    sender: mpsc::UnboundedSender<ApiRequest>,
}

async fn process_cmd(
    State(state): State<ApiEndpointServer>,
    Path(path): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    let (tx, rx) = oneshot::channel::<ApiResponse>();
    let _ = state.sender.send(ApiRequest {
        cmd: path,
        arguments: payload,
        rx: tx,
    });

    match rx.await {
        Ok(response) => (response.code, Json(response.response)),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({}))),
    }
}

#[tokio::main]
async fn serve(
    sender: mpsc::UnboundedSender<ApiRequest>,
    port: u16,
    prefix: &str,
    ready: oneshot::Sender<Result<String, IxaError>>,
) {
    let state = ApiEndpointServer { sender };

    // run our app with Axum, listening on `port`
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await;
    if listener.is_err() {
        ready
            .send(Err(IxaError::IxaError(format!("Could not bind to {port}"))))
            .unwrap();
        return;
    }

    // build our application with a route
    let path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), "static/");
    let static_assets_path = std::path::Path::new(&path);
    let home_path = format!("/{prefix}/static/index.html");
    let app = Router::new()
        .route(&format!("/{prefix}/cmd/{{command}}"), post(process_cmd))
        .route(
            &format!("/{prefix}/"),
            get(|| async move { Redirect::temporary(&home_path) }),
        )
        .nest_service(
            &format!("/{prefix}/static/"),
            ServeDir::new(static_assets_path),
        )
        .nest_service(
            "/favicon.ico",
            ServeFile::new_with_mime(
                static_assets_path.join(std::path::Path::new("favicon.ico")),
                &mime::IMAGE_PNG,
            ),
        )
        .with_state(state);

    // Notify the caller that we are ready.
    ready
        .send(Ok(format!("http://127.0.0.1:{port}/{prefix}/")))
        .unwrap();
    axum::serve(listener.unwrap(), app).await.unwrap();
}

/// Starts the Web API, pausing execution until instructed
/// to continue.
fn handle_web_api(context: &mut Context, api: &mut ApiData) {
    while let Some(req) = api.receiver.blocking_recv() {
        if req.cmd == "continue" {
            let _ = req.rx.send(ApiResponse {
                code: StatusCode::OK,
                response: json!({}),
            });
            break;
        }

        let handler = api.handlers.get(&req.cmd);
        if handler.is_none() {
            let _ = req.rx.send(ApiResponse {
                code: StatusCode::NOT_FOUND,
                response: json!({
                    "error" : format!("No command {}", req.cmd)
                }),
            });
            continue;
        }

        let handler = handler.unwrap();
        match handler(context, req.arguments.clone()) {
            Err(err) => {
                let _ = req.rx.send(ApiResponse {
                    code: StatusCode::BAD_REQUEST,
                    response: json!({
                        "error" : err.to_string()
                    }),
                });
                continue;
            }
            Ok(response) => {
                let _ = req.rx.send(ApiResponse {
                    code: StatusCode::OK,
                    response,
                });
            }
        };
    }
}

pub trait ContextWebApiExt {
    /// Set up the Web API and start the Web server.
    ///
    /// # Errors
    /// `IxaError` on failure to bind to `port`
    fn setup_web_api(&mut self, port: u16) -> Result<String, IxaError>;

    /// Schedule the simulation to pause at time t and listen for
    /// requests from the Web API.
    fn schedule_web_api(&mut self, t: f64);

    /// Add an API point.
    /// # Errors
    /// `IxaError` when the Web API has not been set up yet.
    fn add_web_api_handler(
        &mut self,
        name: &str,
        handler: impl Fn(&mut Context, serde_json::Value) -> Result<serde_json::Value, IxaError>
            + 'static,
    ) -> Result<(), IxaError>;
}

impl ContextWebApiExt for Context {
    fn setup_web_api(&mut self, port: u16) -> Result<String, IxaError> {
        // TODO(cym4@cdc.gov): Check on the limits here.
        let (api_to_ctx_send, api_to_ctx_recv) = mpsc::unbounded_channel::<ApiRequest>();

        let data_container = self.get_data_container_mut(ApiPlugin);
        if data_container.is_some() {
            return Err(IxaError::IxaError(String::from(
                "HTTP API already initialized",
            )));
        }

        // Start the API server
        let mut random: [u8; 16] = [0; 16];
        rand::rngs::OsRng.fill_bytes(&mut random);
        let secret = uuid::Builder::from_random_bytes(random)
            .into_uuid()
            .to_string();

        let (ready_tx, ready_rx) = oneshot::channel::<Result<String, IxaError>>();
        thread::spawn(move || serve(api_to_ctx_send, port, &secret, ready_tx));
        let url = ready_rx.blocking_recv().unwrap()?;

        let mut api_data = ApiData {
            receiver: api_to_ctx_recv,
            handlers: HashMap::new(),
        };

        register_api_handler::<global_properties::Api, global_properties::Args>(
            &mut api_data,
            "global",
        );
        register_api_handler::<population::Api, EmptyArgs>(&mut api_data, "population");
        register_api_handler::<breakpoint::Api, breakpoint::Args>(&mut api_data, "next");
        register_api_handler::<people::Api, people::Args>(&mut api_data, "people");
        register_api_handler::<time::Api, EmptyArgs>(&mut api_data, "time");
        // Record the data container.
        *data_container = Some(api_data);

        Ok(url)
    }

    fn schedule_web_api(&mut self, t: f64) {
        self.add_plan(t, |context| {
            run_with_plugin::<ApiPlugin>(context, |context, data_container| {
                handle_web_api(context, data_container.as_mut().unwrap());
            });
        });
    }

    /// Add an API point.
    fn add_web_api_handler(
        &mut self,
        name: &str,
        handler: impl Fn(&mut Context, serde_json::Value) -> Result<serde_json::Value, IxaError>
            + 'static,
    ) -> Result<(), IxaError> {
        let data_container = self.get_data_container_mut(ApiPlugin);

        match data_container {
            Some(dc) => {
                dc.handlers.insert(name.to_string(), Box::new(handler));
                Ok(())
            }
            None => Err(IxaError::IxaError(String::from("Web API not yet set up"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ContextWebApiExt;
    use crate::people::define_person_property;
    use crate::{define_global_property, ContextGlobalPropertiesExt};
    use crate::{Context, ContextPeopleExt};
    use reqwest::StatusCode;
    use serde::Serialize;
    use serde_json::json;
    use std::thread;

    define_global_property!(WebApiTestGlobal, String);
    define_person_property!(Age, u8);
    fn setup() -> (String, Context) {
        let mut context = Context::new();
        let url = context.setup_web_api(33339).unwrap();
        context.schedule_web_api(0.0);
        context
            .set_global_property_value(WebApiTestGlobal, "foobar".to_string())
            .unwrap();
        context.add_person((Age, 1)).unwrap();
        context.add_person((Age, 2)).unwrap();
        context
            .add_web_api_handler("external", |_context, args| Ok(args))
            .unwrap();
        (url, context)
    }

    // Continue the simulation. Note that we don't wait for a response
    // because there is a race condition between sending the final
    // response and program termination.
    fn send_continue(url: &str) {
        let client = reqwest::blocking::Client::new();
        client
            .post(format!("{url}cmd/continue"))
            .json(&{})
            .send()
            .unwrap();
    }

    // Send a request and check the response.
    fn send_request<T: Serialize + ?Sized>(url: &str, cmd: &str, req: &T) -> serde_json::Value {
        let client = reqwest::blocking::Client::new();
        let response = client
            .post(format!("{url}cmd/{cmd}"))
            .json(req)
            .send()
            .unwrap();
        let status = response.status();
        let response = response.json().unwrap();
        println!("{response:?}");
        assert_eq!(status, StatusCode::OK);
        response
    }

    // Send a request and check the response.
    fn send_request_text(url: &str, cmd: &str, req: String) -> reqwest::blocking::Response {
        let client = reqwest::blocking::Client::new();
        client
            .post(format!("{url}cmd/{cmd}"))
            .header("Content-Type", "application/json")
            .body(req)
            .send()
            .unwrap()
    }

    // We do all of the tests in one test block to avoid having to
    // start a lot of servers with different ports and having
    // to manage that. This may not be ideal, but we're doing it for now.
    // TODO(cym4@cdc.gov): Consider using some kind of static
    // object to isolate the test cases.
    #[allow(clippy::too_many_lines)]
    #[test]
    fn web_api_test() {
        #[derive(Serialize)]
        struct PopulationResponse {
            population: usize,
        }

        // TODO(cym4@cdc.gov): If this thread fails
        // then the test will stall instead of
        // erroring out, but there's nothing that
        // should fail here.
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let ctx_thread = thread::spawn(move || {
            let (url, mut context) = setup();
            let _ = tx.send(url);
            context.execute();
        });

        let url = rx.recv().unwrap();
        // Test the population API point.
        let res = send_request(&url, "population", &json!({}));
        assert_eq!(json!(&PopulationResponse { population: 2 }), res);

        // Test the time API point.
        let res = send_request(&url, "time", &json!({}));
        assert_eq!(
            json!(
                { "time": 0.0 }
            ),
            res
        );

        // Test the global property list point. We can't do
        // exact match because the return is every defined
        // global property anywhere in the code.
        let res = send_request(
            &url,
            "global",
            &json!({
                "Global": "List"
            }),
        );
        let list = res.get("List").unwrap().as_array().unwrap();
        let mut found = false;
        for prop in list {
            let prop_val = prop.as_str().unwrap();
            if prop_val == "ixa.WebApiTestGlobal" {
                found = true;
                break;
            }
        }
        assert!(found);

        // Test the global property get API point.
        let res = send_request(
            &url,
            "global",
            &json!({
                "Global": {
                    "Get" : {
                        "property" : "ixa.WebApiTestGlobal"
                    }
                }
            }),
        );
        // The extra quotes here are because we internally JSONify.
        // TODO(cym4@cdc.gov): Should we fix this internally?
        assert_eq!(
            res,
            json!({
                "Value": "\"foobar\""
            })
        );

        // Next time.
        let res = send_request(
            &url,
            "next",
            &json!({
                "Next": {
                    "next_time" : 1.0
                }
            }),
        );
        assert_eq!(res, json!({}));

        // Person properties API.
        let res = send_request(
            &url,
            "people",
            &json!({
                "People" : {
                    "Get" : {
                        "person_id": 0,
                        "property" : "Age"
                    }
                }
            }),
        );
        assert_eq!(
            res,
            json!({"Properties" : [
                ( "Age",  "1" )
            ]}
            )
        );

        // List properties.
        let res = send_request(
            &url,
            "people",
            &json!({
                "People" : "Properties"
            }),
        );
        assert_eq!(
            res,
            json!({"PropertyNames" : [
                "Age"
            ]}
            )
        );

        // Tabulate API.
        let res = send_request(
            &url,
            "people",
            &json!({
                "People" : {
                    "Tabulate" : {
                        "properties": ["Age"]
                    }
                }
            }),
        );

        // This is a hack to deal with these arriving in
        // arbitrary order.
        assert!(
            (res == json!({"Tabulated" : [
                [{ "Age" :  "1" }, 1],
                [{ "Age" :  "2" }, 1]
            ]})) || (res
                == json!({"Tabulated" : [
                    [{ "Age" :  "2" }, 1],
                    [{ "Age" :  "1" }, 1]
                ]})),
        );

        // Valid JSON but wrong type.
        let res = send_request_text(
            &url,
            "next",
            String::from("{\"Next\": {\"next_time\" : \"invalid\"}}"),
        );
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        // Invalid JSON.
        let res = send_request_text(&url, "next", String::from("{]"));
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        // A generic externally added API handler
        let res = send_request(&url, "external", &json!({"External": [1]}));
        assert_eq!(res, json!({"External": [1]}));

        // Test continue and make sure that the context
        // exits.
        send_continue(&url);
        let _ = ctx_thread.join();
    }
}

use crate::context::{run_with_plugin, Context};
use crate::define_data_plugin;
use crate::error::IxaError;
use crate::extension_api::{
    run_extension, EmptyArgs, GlobalPropertyExtension, GlobalPropertyExtensionArgs,
    NextCommandExtension, NextExtensionArgs, PopulationExtension,
};
use axum::extract::{Json, Path, State};
use axum::{http::StatusCode, routing::post, Router};
use serde_json::json;
use std::collections::HashMap;
use std::thread;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

type ApiHandler = dyn Fn(&mut Context, serde_json::Value) -> Result<serde_json::Value, IxaError>;

macro_rules! register_api_handler {
    ($dc:ident, $name:ident, $extension_type:ty, $args_type:ty, $retval_type:ty) => {
        $dc.handlers.insert(
            stringify!($name).to_string(),
            Box::new(
                |context, args_json| -> Result<serde_json::Value, IxaError> {
                    let args: $args_type = serde_json::from_value(args_json)?;
                    let retval = run_extension::<$extension_type>(context, &args)?;
                    Ok(serde_json::to_value(retval)?)
                },
            ),
        );
    };
}

struct ApiData {
    receiver: mpsc::Receiver<ApiRequest>,
    handlers: HashMap<String, Box<ApiHandler>>,
}

define_data_plugin!(ApiPlugin, Option<ApiData>, None);

// Input to the API handler.
struct ApiRequest {
    cmd: String,
    arguments: serde_json::Value,
    rx: oneshot::Sender<ApiResponse>,
}

// Output of the API handler.
struct ApiResponse {
    success: bool,
    response: serde_json::Value,
}

#[derive(Clone)]
struct ApiEndpointServer {
    sender: mpsc::Sender<ApiRequest>,
}

async fn process_cmd(
    State(state): State<ApiEndpointServer>,
    Path(path): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    let (tx, rx) = oneshot::channel::<ApiResponse>();

    let _ = state
        .sender
        .send(ApiRequest {
            cmd: path,
            arguments: payload,
            rx: tx,
        })
        .await;

    match rx.await {
        Ok(ApiResponse {
            success: true,
            response,
        }) => (StatusCode::OK, Json(response)),
        Ok(ApiResponse {
            success: false,
            response,
        }) => (StatusCode::BAD_REQUEST, Json(response)),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({}))),
    }
}

#[tokio::main]
async fn serve(
    sender: mpsc::Sender<ApiRequest>,
    port: u16,
    ready: std::sync::mpsc::Sender<Result<(), IxaError>>,
) {
    let state = ApiEndpointServer { sender };

    // run our app with Axum, listening globally on port
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await;
    if listener.is_err() {
        ready
            .send(Err(IxaError::IxaError(format!("Could not bind to {port}"))))
            .unwrap();
        return;
    }

    // build our application with a route
    let app = Router::new()
        .route("/cmd/{command}", post(process_cmd))
        .with_state(state);

    // Notify the caller that we are ready.
    ready.send(Ok(())).unwrap();
    axum::serve(listener.unwrap(), app).await.unwrap();
}

/// Starts the Web API and pauses execution
fn handle_web_api(context: &mut Context, api: &mut ApiData) -> Result<(), IxaError> {
    loop {
        let req = api.receiver.blocking_recv();

        if req.is_none() {
            continue;
        }

        let req = req.unwrap();
        if req.cmd == "continue" {
            let _ = req.rx.send(ApiResponse {
                success: true,
                response: json!({}),
            });
            break;
        }

        let handler = api.handlers.get(&req.cmd);
        if handler.is_none() {
            let _ = req.rx.send(ApiResponse {
                success: false,
                response: json!({
                    "error" : format!("No command {}", req.cmd)
                }),
            });
            continue;
        }

        let handler = handler.unwrap();
        match handler(context, req.arguments) {
            Err(err) => {
                let _ = req.rx.send(ApiResponse {
                    success: false,
                    response: json!({
                        "error" : err.to_string()
                    }),
                });
            }
            Ok(response) => {
                let _ = req.rx.send(ApiResponse {
                    success: true,
                    response,
                });
            }
        };
    }

    Ok(())
}

pub trait ContextWebApiExt {
    /// Set up the Web API and start the Web server.
    fn setup_web_api(&mut self, port: u16) -> Result<(), IxaError>;

    /// Schedule the simulation to pause at time t and listen for
    /// requests from the Web API.
    ///
    /// # Errors
    /// Internal debugger errors e.g., reading or writing to stdin/stdout;
    /// errors in Ixa are printed to stdout
    fn schedule_web_api(&mut self, t: f64);
}

impl ContextWebApiExt for Context {
    fn setup_web_api(&mut self, port: u16) -> Result<(), IxaError> {
        // TODO(cym4@cdc.gov): Check on the limits here.
        let (api_to_ctx_send, api_to_ctx_recv) = mpsc::channel::<ApiRequest>(32);

        let data_container = self.get_data_container_mut(ApiPlugin);
        if data_container.is_some() {
            return Err(IxaError::IxaError(String::from(
                "HTTP API already initialized",
            )));
        }

        // Start the API server
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), IxaError>>();
        thread::spawn(move || serve(api_to_ctx_send, port, ready_tx));
        let ready = ready_rx.recv().unwrap();
        if ready.is_err() {
            return ready;
        }

        let mut api_data = ApiData {
            receiver: api_to_ctx_recv,
            handlers: HashMap::new(),
        };

        register_api_handler!(
            api_data,
            global,
            GlobalPropertyExtension,
            GlobalPropertyExtensionArgs,
            GlobalPropertyExtensionRetval
        );

        register_api_handler!(
            api_data,
            population,
            PopulationExtension,
            EmptyArgs,
            PopulationExtensionRetval
        );

        register_api_handler!(
            api_data,
            next,
            NextCommandExtension,
            NextExtensionArgs,
            NextExtensionRetval
        );

        // Record the data container.
        *data_container = Some(api_data);

        Ok(())
    }

    fn schedule_web_api(&mut self, t: f64) {
        self.add_plan(t, |context| {
            run_with_plugin::<ApiPlugin>(context, |context, data_container| {
                println!("Paused in Web API");
                handle_web_api(context, data_container.as_mut().unwrap())
                    .expect("Error in Web API");
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::ContextWebApiExt;
    use crate::{define_global_property, ContextGlobalPropertiesExt};
    use crate::{Context, ContextPeopleExt};
    use reqwest::StatusCode;
    use serde::Serialize;
    use std::thread;

    #[derive(Serialize)]
    struct EmptyArgs {}

    define_global_property!(WebApiTestGlobal, String);

    fn setup_context() -> Context {
        let mut context = Context::new();
        context.setup_web_api(33339).unwrap();
        context.schedule_web_api(0.0);
        context
            .set_global_property_value(WebApiTestGlobal, "foobar".to_string())
            .unwrap();
        context.add_person(()).unwrap();
        context.add_person(()).unwrap();
        context
    }

    // Continue the simulation. Note that we don't wait for a response
    // because there is a race condition between sending the final
    // response and program termination.
    fn send_continue() {
        let client = reqwest::blocking::Client::new();
        client
            .post("http://127.0.0.1:33339/cmd/continue")
            .json("")
            .send()
            .unwrap();
    }

    // Send a request and check the response.
    fn send_request<T: Serialize + ?Sized, U: Serialize + ?Sized>(cmd: &str, req: &T, res: &U) {
        let client = reqwest::blocking::Client::new();
        let response = client
            .post(&format!("http://127.0.0.1:33339/cmd/{cmd}"))
            .json(req)
            .send()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            serde_json::to_string(res).unwrap(),
            response.text().unwrap()
        );
    }

    // We do all of the tests in one test block to avoid having to
    // start a lot of servers with different ports and having
    // to manage that. This may not be ideal, but we're doing it for now.
    // TODO(cym4@cdc.gov): Consider using some kind of static
    // object to isolate the test cases.

    #[test]
    fn web_api_get_population() {
        #[derive(Serialize)]
        struct PopulationResponse {
            population: usize,
        }

        // TODO(cym4@cdc.gov): If this thread fails
        // then the test will stall instead of
        // erroring out, but there's nothing that
        // should fail here.
        let ctx_thread = thread::spawn(|| {
            let mut context = setup_context();
            context.execute();
        });

        // Test the population API point.
        send_request(
            &"population",
            &EmptyArgs {},
            &PopulationResponse { population: 2 },
        );

        // Test continue and make sure that the context
        // exits.
        send_continue();
        let _ = ctx_thread.join();
    }
}

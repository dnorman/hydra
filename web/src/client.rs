use futures::future::{select, Either, FutureExt};
use futures::io::Read;
use futures::select;
use futures_signals::signal::{Mutable, SignalExt};
use futures_signals::signal::{MutableSignal, ReadOnlyMutable};
use gloo_timers::future::sleep;
use log::{error, info, warn};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{CloseEvent, Event, MessageEvent, WebSocket};

#[wasm_bindgen(start)]
pub async fn start() -> Result<(), JsValue> {
    wasm_logger::init(wasm_logger::Config::default());
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ConnectionState {
    None,
    Connecting,
    Open,
    Closed,
    Error,
}

struct ClientInner {
    connection: RefCell<Option<Connection>>,
    state: Mutable<ConnectionState>,
}

#[wasm_bindgen]
pub struct Client {
    inner: Rc<ClientInner>,
}

#[wasm_bindgen]
impl Client {
    pub fn new() -> Result<Client, JsValue> {
        let inner = Rc::new(ClientInner {
            connection: RefCell::new(None),
            state: Mutable::new(ConnectionState::None),
        });

        inner.connect()?;

        Ok(Client { inner })
    }
    pub async fn ready(&self) {
        self.inner
            .state
            .signal()
            .wait_for(ConnectionState::Open)
            .await;
    }
    pub fn send_message(&self, message: &str) {
        info!("send_message: Sending message: {}", message);
        self.inner
            .connection
            .borrow_mut()
            .as_ref()
            .unwrap()
            .send_message(message);
    }
}

impl ClientInner {
    pub fn connect(self: &Rc<Self>) -> Result<(), JsValue> {
        let connection = Connection::new()?;
        let state = connection.state.clone();
        self.connection.borrow_mut().replace(connection);

        self.state.set(ConnectionState::Connecting);
        let client_inner = Rc::clone(&self);

        info!("Connecting to websocket");
        spawn_local(async move {
            select! {
                _ = sleep(Duration::from_secs(30)).fuse() => {
                    if state.get() != ConnectionState::Open {
                        warn!("connect: Connection timed out");
                        client_inner.connect().expect("Failed to reconnect");
                    }
                }
                _ = state.signal().wait_for(ConnectionState::Open).fuse() => {
                    info!("connect: Connection opened");
                    state.signal().for_each(|state| {
                        info!("connect: state changed to {:?}", state);
                            client_inner.state.set(state);
                            // if state isn't open or connecting, reconnect
                            if state != ConnectionState::Open && state != ConnectionState::Connecting {
                                client_inner.connect().expect("Failed to reconnect");
                            }
                            futures::future::ready(())
                    }).await;

                    info!("for_each future complete");
                }
            };
        });

        Ok(())
    }
}

struct Connection {
    ws: WebSocket,
    on_message: Closure<dyn FnMut(MessageEvent)>,
    on_error: Closure<dyn FnMut(Event)>,
    on_close: Closure<dyn FnMut(CloseEvent)>,
    on_open: Closure<dyn FnMut()>,
    state: ReadOnlyMutable<ConnectionState>,
}

impl Connection {
    fn new() -> Result<Connection, JsValue> {
        let ws = WebSocket::new("ws://127.0.0.1:9797/ws")?;

        let writable_state = Mutable::new(ConnectionState::Connecting);
        let writable_state2 = writable_state.clone();
        let writable_state3 = writable_state.clone();
        let state = writable_state.read_only();
        let on_message =
            Closure::<dyn FnMut(MessageEvent)>::wrap(Box::new(move |e: MessageEvent| {
                if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                    info!("Message received: {}", text);
                }
            }));

        let on_error = Closure::<dyn FnMut(Event)>::wrap(Box::new(move |_| {
            info!("Connection Error");
            writable_state.set(ConnectionState::Error);
        }));

        let on_close = Closure::<dyn FnMut(CloseEvent)>::wrap(Box::new(move |e: CloseEvent| {
            info!("Connection closed: {}", e.code());
            writable_state2.set(ConnectionState::Closed);
        }));

        // convert ready into a future
        let on_open = Closure::<dyn FnMut()>::wrap(Box::new(move || {
            info!("Connection opened (event)");
            writable_state3.set(ConnectionState::Open);
        }));

        // Set up WebSocket event handlers
        ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));

        Ok(Connection {
            ws,
            on_message,
            on_error,
            on_close,
            on_open,
            state,
        })
    }

    pub fn send_message(&self, message: &str) {
        self.ws.send_with_str(message).unwrap_or_else(|err| {
            info!("Failed to send message: {:?}", err);
        });
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        info!("Dropping connection");
        // unbind the listeners and close the connection
        self.ws.set_onmessage(None);
        self.ws.set_onerror(None);
        self.ws.set_onclose(None);
        self.ws.close().unwrap();
    }
}

use futures_signals::signal::{Mutable, SignalExt};
use futures_signals::signal::{MutableSignal, ReadOnlyMutable};
use log::info;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CloseEvent, Event, MessageEvent, WebSocket};

#[wasm_bindgen(start)]
pub async fn start() -> Result<(), JsValue> {
    wasm_logger::init(wasm_logger::Config::default());
    Ok(())
}

#[wasm_bindgen]
pub struct Client {
    ws: WebSocket,
    on_message: Closure<dyn FnMut(MessageEvent)>,
    on_error: Closure<dyn FnMut(Event)>,
    on_close: Closure<dyn FnMut(CloseEvent)>,
    on_open: Closure<dyn FnMut()>,
    ready: ReadOnlyMutable<bool>,
}

#[wasm_bindgen]
impl Client {
    pub fn new() -> Result<Client, JsValue> {
        let ws = WebSocket::new("ws://127.0.0.1:9797/ws")?;

        let on_message =
            Closure::<dyn FnMut(MessageEvent)>::wrap(Box::new(move |e: MessageEvent| {
                if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                    info!("Message received: {}", text);
                }
            }));

        let on_error = Closure::<dyn FnMut(Event)>::wrap(Box::new(|_| {
            info!("Connection Error");
        }));

        let on_close = Closure::<dyn FnMut(CloseEvent)>::wrap(Box::new(move |e: CloseEvent| {
            info!("Connection closed: {}", e.code());
        }));

        let ready = Mutable::new(false);
        let ready_signal = ready.read_only();
        // convert ready into a future
        let on_open = Closure::<dyn FnMut()>::wrap(Box::new(move || {
            info!("Connection opened");
            ready.set(true);
        }));

        // Set up WebSocket event handlers
        ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));

        Ok(Client {
            ws,
            on_message,
            on_error,
            on_close,
            on_open,
            ready: ready_signal,
        })
    }

    pub async fn ready(&self) {
        self.ready.signal().wait_for(true).await;
    }

    pub fn send_message(&self, message: &str) {
        self.ws.send_with_str(message).unwrap_or_else(|err| {
            info!("Failed to send message: {:?}", err);
        });
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        info!("Dropping client");
        // unbind the listeners and close the connection
        self.ws.set_onmessage(None);
        self.ws.set_onerror(None);
        self.ws.set_onclose(None);
        self.ws.close().unwrap();
    }
}

use log::info;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

#[wasm_bindgen(start)]
pub async fn start() -> Result<(), JsValue> {
    wasm_logger::init(wasm_logger::Config::default());
    Ok(())
}

pub struct Client {
    ws: WebSocket,
    on_message: Closure<dyn FnMut(MessageEvent)>,
    on_error: Closure<dyn FnMut(ErrorEvent)>,
    on_close: Closure<dyn FnMut(CloseEvent)>,
}

impl Client {
    pub fn new() -> Result<Client, JsValue> {
        let ws = WebSocket::new("ws://127.0.0.1:9797")?;

        let on_message =
            Closure::<dyn FnMut(MessageEvent)>::wrap(Box::new(move |e: MessageEvent| {
                if let Ok(text) = e.data().dyn_into::<js_sys::JsString>() {
                    info!("Message received: {}", text);
                }
            }));

        let on_error = Closure::<dyn FnMut(ErrorEvent)>::wrap(Box::new(move |e: ErrorEvent| {
            info!("Error: {}", e.message());
        }));

        let on_close = Closure::<dyn FnMut(CloseEvent)>::wrap(Box::new(move |e: CloseEvent| {
            info!("Connection closed: {}", e.code());
        }));

        // Set up WebSocket event handlers
        ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));

        Ok(Client {
            ws,
            on_message,
            on_error,
            on_close,
        })
    }

    pub fn send_message(&self, message: &str) {
        self.ws.send_with_str(message).unwrap_or_else(|err| {
            info!("Failed to send message: {:?}", err);
        });
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        // unbind the listeners and close the connection
        self.ws.set_onmessage(None);
        self.ws.set_onerror(None);
        self.ws.set_onclose(None);
        self.ws.close().unwrap();
    }
}

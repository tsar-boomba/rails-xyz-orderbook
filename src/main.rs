mod client;
mod models;
mod processing;

use fastwebsockets::FragmentCollector;
use http_body_util::{BodyExt, Empty};
use hyper::Request;

use client::Client;

use crate::processing::start_processing;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install().unwrap();

    dotenvy::dotenv()?;

    let client = Client::new()?;

    let (ws, _res) = client
        .websocket(
            "wss://order-stream.trade.rails.xyz?market=ETH-USD".parse()?,
            Request::builder().body(Empty::new().boxed())?,
        )
        .await?;
    let mut ws = FragmentCollector::new(ws);

    // Wait for first "orderBook" message
    let mut orders: Option<Vec<models::RawOrder>> = None;
    let mut completed_orders: Option<Vec<models::RawCompletedOrder>> = None;
    let (orders, completed_orders) = loop {
        let mut frame = ws.read_frame().await?;

        match frame.opcode {
            fastwebsockets::OpCode::Text => {
                if orders.is_none() && is_message_type(&frame.payload, b"orderBook") {
                    println!("got initial order book");
                    let payload = frame.payload.to_mut();
                    let payload_len = payload.len();
                    let orders_array_start = 15 + "orderBook".len() + 36;
                    orders = Some(simd_json::from_slice(
                        &mut payload[orders_array_start..(payload_len - 2)],
                    )?);
                } else if completed_orders.is_none()
                    && is_message_type(&frame.payload, b"completedOrders")
                {
                    println!("got initial completed orders");
                    let payload = frame.payload.to_mut();
                    let payload_len = payload.len();
                    let orders_array_start = 15 + "completedOrders".len() + 36;
                    completed_orders = Some(simd_json::from_slice(
                        &mut payload[orders_array_start..(payload_len - 2)],
                    )?);
                }
            }
            fastwebsockets::OpCode::Close => return Ok(()),
            _ => {}
        }

        if orders.is_some() && completed_orders.is_some() {
            break (orders.unwrap(), completed_orders.unwrap());
        }
    };

    let (order_delta_sender, completed_delta_sender) = start_processing(orders, completed_orders);

    loop {
        let mut frame = ws.read_frame().await?;

        match frame.opcode {
            fastwebsockets::OpCode::Text => {
                if is_message_type(&frame.payload, b"orderBookDelta") {
                    let payload = frame.payload.to_mut();
                    let payload_len = payload.len();
                    let orders_array_start = 15 + "orderBookDelta".len() + 36;
                    let orders_delta: Vec<models::RawOrder> =
                        simd_json::from_slice(&mut payload[orders_array_start..(payload_len - 2)])?;
                    order_delta_sender.send(orders_delta).await?;
                } else if is_message_type(&frame.payload, b"completedOrdersDelta") {
                    let payload = frame.payload.to_mut();
                    let payload_len = payload.len();
                    let orders_array_start = 15 + "completedOrdersDelta".len() + 36;
                    let completed_orders_delta: Vec<models::RawCompletedOrder> =
                        simd_json::from_slice(&mut payload[orders_array_start..(payload_len - 2)])?;
                    completed_delta_sender.send(completed_orders_delta).await?;
                }
            }
            fastwebsockets::OpCode::Close => break,
            _ => {}
        }
    }

    Ok(())
}

fn is_message_type(message: &[u8], desired_type: &[u8]) -> bool {
    &message[15..(15 + desired_type.len())] == desired_type
    // double quote character must be after the desired type
        && message[15 + desired_type.len()] == b'"'
}

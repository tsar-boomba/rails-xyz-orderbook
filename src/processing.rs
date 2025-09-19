use std::{
    collections::BTreeMap,
    io::{BufWriter, Write, stdout},
};

use crate::models;
use float_ord::FloatOrd;

type OrderBook = BTreeMap<FloatOrd<f32>, f32>;

pub fn start_processing(
    orders: Vec<models::RawOrder>,
    completed_orders: Vec<models::RawCompletedOrder>,
) -> (
    kanal::AsyncSender<Vec<models::RawOrder>>,
    kanal::AsyncSender<Vec<models::RawCompletedOrder>>,
) {
    let (orders_sender, orders_receiver) = kanal::bounded::<Vec<models::RawOrder>>(16);
    let (completed_sender, completed_receiver) =
        kanal::bounded::<Vec<models::RawCompletedOrder>>(16);

    std::thread::spawn(move || {
        println!("{orders:#?}");
        let mut bid_order_book = OrderBook::default();
        let mut offer_order_book = OrderBook::default();

        for order in orders {
            if order.order_type == models::OrderType::Buy {
                bid_order_book.insert(
                    FloatOrd(order.price.parse::<f32>().unwrap()),
                    order.quantity.parse().unwrap(),
                );
            } else if order.order_type == models::OrderType::Sell {
                offer_order_book.insert(
                    FloatOrd(order.price.parse::<f32>().unwrap()),
                    order.quantity.parse().unwrap(),
                );
            }
        }

        while let Ok(order_deltas) = orders_receiver.recv() {
            for delta in order_deltas {
                if &delta.quantity == "0" {
                    if delta.order_type == models::OrderType::Buy {
                        bid_order_book.remove(&FloatOrd(delta.price.parse::<f32>().unwrap()));
                    } else if delta.order_type == models::OrderType::Sell {
                        offer_order_book.remove(&FloatOrd(delta.price.parse::<f32>().unwrap()));
                    }
                } else {
                    if delta.order_type == models::OrderType::Buy {
                        bid_order_book.insert(
                            FloatOrd(delta.price.parse::<f32>().unwrap()),
                            delta.quantity.parse().unwrap(),
                        );
                    } else if delta.order_type == models::OrderType::Sell {
                        offer_order_book.insert(
                            FloatOrd(delta.price.parse::<f32>().unwrap()),
                            delta.quantity.parse().unwrap(),
                        );
                    }
                }
            }

            print_book(&bid_order_book, "Bids");
            print_book(&offer_order_book, "Offers");
        }
    });

    std::thread::spawn(move || {
        println!("{completed_orders:#?}");
        let mut deltas = 0u64;
        while let Ok(_completed_deltas) = completed_receiver.recv() {
            deltas += 1;

            // TODO: process completed order deltas

            if deltas % 10 == 0 {
                println!("received {deltas} completed deltas");
            }
        }
    });

    (orders_sender.clone_async(), completed_sender.clone_async())
}

fn print_book(book: &OrderBook, name: &str) {
    let mut stdout = BufWriter::new(stdout().lock());
    writeln!(stdout, "===== {name} =====").unwrap();
    for (k, v) in book {
        writeln!(stdout, "{:.2}: {}", k.0, v).unwrap();
    }
    writeln!(stdout, "===== End {name} =====").unwrap();
    stdout.flush().unwrap();
}

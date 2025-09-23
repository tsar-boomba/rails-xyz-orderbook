# Rails.xyz Order Book Monitor

Haiiii :3 I'm Isaiah "Ibomb" Gamble and this is my warm up project for Trading @GT QD!!

I decided to take a look at the [rails.xyz](rails.xyz) exchange.

## Process

1. Used tokio, hyper, and fastwebsockets to establish the connection
2. Quickly parsed JSON messages by checking which type the message was and directly slicing out the JSON array for simd-json to deserialize
3. Send deltas to the processing thread to update the bid and offer books, then print the updated books

## Running

You should be able to run this on any platform with:

```sh
cargo run --release
```

## Questions

?

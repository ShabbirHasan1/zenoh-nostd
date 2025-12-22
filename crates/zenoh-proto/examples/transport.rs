use zenoh_proto::{msgs::*, *};

fn session_process<'a, 'b>(
    session: &'a (),
    x: impl Iterator<Item = NetworkMessage<'b>>,
) -> impl Iterator<Item = NetworkMessage<'a>> {
    core::iter::from_fn(move || {
        let _ = (session, &x);
        None
    })
}

fn main() {
    let session = ();

    // Create a transport using stack-allocated buffers.
    // Heap-allocated buffers can be used as well.
    let mut transport = Transport::new([0u8; 512]).batch_size(256).streamed();

    loop {
        // Retrieve the local state of the transport. It's needed for storing stateful interactions.
        // This kind of storage must be provided by the user of the transport. And must be constructed on each
        // iteration.
        let mut local = transport.state.local().expect("Incorrect state");
        // Feed raw bytes into the transport.
        // Several methods are available:
        // - `transport.feed(&[u8])` copies data from an existing slice.
        // - `transport.feed_with(|buf| { ... })` allows filling the internal buffer directly.
        // - `transport.feed_exact(len, |buf| { ... })` fills exactly `len` bytes.
        //
        // **Note** in streamed mode, the `feed_with(|buf| {})` will be called twice: the first one
        // to retrieve the len of the payload, the last one to retrieve the payload.
        transport.rx.feed(&mut local, &[1, 2, 3, 4, 5]);

        // Flush the RX side of the transport to process received data.
        // This decodes messages and updates the internal transport state.
        // It returns an iterator over session-level messages.
        // No actual processing happens until the iterator is consumed.
        let network_msgs = transport.rx.flush(&mut local);

        // Process session messages and produce a response.
        // As before, processing is lazy: nothing happens until
        // the returned iterator is consumed.
        let session_resp = session_process(&session, network_msgs);

        // Send the response using the TX side of the transport.
        // This is where data is actually consumed and the transport
        // (and possibly the session) state is updated.
        // The iterator yields byte slices ready to be sent over the
        // network or any other transport medium.
        for chunk in transport.tx.write(session_resp) {
            println!("Sent chunk: {:?}", chunk);
        }

        // Perform auxiliary transport interactions, such as:
        // - sending keep-alive messages;
        // - gracefully closing the connection;
        // - responding to InitSyn / OpenSyn;
        // - initiating a handshake, etc.
        let interact = transport.tx.interact(&mut local);
        println!("Sent interact: {:?}", interact);
    }
}

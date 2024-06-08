TODO:

- Specify proxy's identity.

- Heavy `Vec` copy operations may hinder performance.

- Make responses streaming (impossible due to caching?)

- Implement file-persistency for in-memory DB. Also, save on `SIG{INT,TERM}`.

- Redis support.

- Incrementing nonce to avoid upstream request replay attack.

- No need to hash the entire request, just the nonce.

- Ensure using parking_lot Mutex rather than std any and `.future_lock()`.

- If the proxy is directed to its own URL, will this work as a DoS attack?

- Improve tests:

    - Use Docker `ENTRYPOINT` to call cargo tests.
    
    - Output which test is running now.

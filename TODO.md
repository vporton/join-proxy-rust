TODO:

- Heavy `Vec` copy operations may hinder performance.

- Make responses streaming.

- Implement file-persistency for in-memory DB. Also, save on `SIG{INT,TERM}`.

- Redis support.

- Incrementing nonce to avoid upstream request replay attack.

- No need to MD5 the entire request, just the nonce.

- Ensure using parking_lot Mutex rather than std any and `.future_lock()`.
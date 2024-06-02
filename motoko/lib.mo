import Types "HttpTypes";
import Itertools "mo:itertools/Iter";
import Sha256 "mo:sha2/Sha256";
import Text "mo:base/Text";
import Iter "mo:base/Iter";
import Debug "mo:base/Debug";
import Blob "mo:base/Blob";
import Buffer "mo:base/Buffer";
import Char "mo:base/Char";
import Nat8 "mo:base/Nat8";
import Nat32 "mo:base/Nat32";
import Time "mo:base/Time";
import Int "mo:base/Int";
import BTree "mo:stableheapbtreemap/BTree";

module {
    public func serializeHttpRequest(request: Types.HttpRequestArgs): Blob {
        let method = switch(request.method) {
            case(#get) { "GET" };
            case(#post) { "POST" };
            case(#head) { "HEAD" };
        };
        let headers_list = Iter.map<Types.HttpHeader, Text>(
            request.headers.vals(),
            func ({name: Text; value: Text}) { name # "\t" # value });
        let headers_joined = Itertools.reduce<Text>(headers_list, func(a: Text, b: Text) {a # "\r" # b});
        let headers_joined2 = switch (headers_joined) {
            case (?s) s;
            case null "";
        };
        let header_part = method # "\n" # request.url # "\n" # headers_joined2;

        let body = switch(request.body) {
            case (?body) { body };
            case null { [] };
        };
        let result = Buffer.Buffer<Nat8>(header_part.size() + 1 + body.size());
        result.append(Buffer.fromArray(Blob.toArray(Text.encodeUtf8(header_part))));
        result.add(Nat8.fromNat(Nat32.toNat(Char.toNat32('\n'))));
        result.append(Buffer.fromArray(body));
        Blob.fromArray(Buffer.toArray(result));
    };

    public func hashOfHttpRequest(request: Types.HttpRequestArgs): Blob {
        // TODO: space inefficient
        let blob = serializeHttpRequest(request);
        Sha256.fromBlob(#sha256, blob);
    };

    type HttpRequestsChecker = {
        hashes: BTree.BTree<Blob, Int>; // hash -> time
        times: BTree.BTree<Int, BTree.BTree<Blob, ()>>;
    };

    public func newHttpRequestsChecker(): HttpRequestsChecker {
        {
            hashes = BTree.init(null);
            times = BTree.init(null);
        }
    };

    private func deleteOldHttpRequests(checker: HttpRequestsChecker, params: {timeout: Nat}) {
        let threshold = Time.now() - params.timeout;
        label r loop {
            let ?(minTime, hashes) = BTree.min(checker.times) else {
                break r;
            };
            if (minTime > threshold) {
                break r;
            };
            for ((hash, _) in BTree.entries(hashes)) {
                ignore BTree.delete(checker.hashes, Blob.compare, hash);
            };
            ignore BTree.delete(checker.times, Int.compare, minTime);
        };
    };

    public func announceHttpRequestHash(checker: HttpRequestsChecker, hash: Blob, params: {timeout: Nat}) {
        deleteOldHttpRequests(checker, params);

        // If there is an old hash equal to this, first delete it to clean times:
        switch (BTree.get(checker.hashes, Blob.compare, hash)) {
            case (?oldTime) {
                let ?subtree = BTree.get(checker.times, Int.compare, oldTime) else {
                    Debug.trap("programming error: zero times");
                };
                ignore BTree.delete(checker.hashes, Blob.compare, hash);
                if (BTree.size(subtree) == 1) {
                    ignore BTree.delete(checker.times, Int.compare, oldTime);
                } else {
                    ignore BTree.delete(subtree, Blob.compare, hash);
                };
            };
            case null {};
        };

        let now = Time.now();

        // Insert into two trees:
        ignore BTree.insert(checker.hashes, Blob.compare, hash, now);
        let subtree = switch (BTree.get(checker.times, Int.compare, now)) {
            case (?hashes) hashes;
            case (null) {
                let hashes = BTree.init<Blob, ()>(null);
                ignore BTree.insert(checker.times, Int.compare, now, hashes);
                hashes;
            }
        };
        ignore BTree.insert(subtree, Blob.compare, hash, ());
        Debug.print(debug_show(BTree.min(checker.hashes)) # " - our hash after insert."); // TODO: Remove.
    };

    public func announceHttpRequest(checker: HttpRequestsChecker, request: Types.HttpRequestArgs, params: {timeout: Nat}) {
        announceHttpRequestHash(checker, hashOfHttpRequest(request), params);
    };

    public func checkHttpRequest(checker: HttpRequestsChecker, hash: Blob): Bool {
        Debug.print(debug_show(BTree.min(checker.hashes)) # " - our min hash."); // TODO: Remove.
        Debug.print(debug_show(hash) # " - asked hash."); // TODO: Remove.
        BTree.has(checker.hashes, Blob.compare, hash);
    };

    public func checkedHttpRequest(checker: HttpRequestsChecker, request: Types.HttpRequestArgs, params: {timeout: Nat}): async* Types.HttpResponsePayload {
        announceHttpRequest(checker, request, params);
        Debug.print(debug_show(BTree.min(checker.hashes)) # " - our min hash 2."); // TODO: Remove.
        await Types.ic.http_request(request);
    };
};
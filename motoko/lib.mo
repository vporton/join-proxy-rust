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
import RBTree "mo:base/RBTree";
import Option "mo:base/Option";

module {
    public type HttpMethod = { #get; #post; #head };

    public type HttpHeaders = RBTree.RBTree<Text, [Text]>;

    public type HttpRequest = {
        method: HttpMethod;
        headers: HttpHeaders;
        url: Text;
        body: Blob;
    };

    func httpMethodToText(method: HttpMethod): Text {
        switch(method) {
            case(#get) { "GET" };
            case(#post) { "POST" };
            case(#head) { "HEAD" };
        };
    };

    public func serializeHttpRequest(request: HttpRequest): Blob {
        let method = httpMethodToText(request.method);
        let headers_list = Iter.map<(Text, [Text]), Text>(
            request.headers.entries(),
            func (entry: (Text, [Text])) { entry.0 # "\t" # Text.join("\t", entry.1.vals()); });
        let headers_joined = Itertools.reduce<Text>(headers_list, func(a: Text, b: Text) {a # "\r" # b});
        let headers_joined2 = switch (headers_joined) {
            case (?s) s;
            case null "";
        };
        let the_rest = Itertools.skip(request.url.chars(), 8); // strip "https://"
        let url = Text.fromIter(Itertools.skipWhile<Char>(the_rest, func (c: Char) { c != '/' }));
        // Debug.print("URL[" # url # "]"); // TODO: Remove.
        let header_part = method # "\n/" # url # "\n" # headers_joined2;

        let result = Buffer.Buffer<Nat8>(header_part.size() + 1 + request.body.size());
        result.append(Buffer.fromArray(Blob.toArray(Text.encodeUtf8(header_part))));
        result.add(Nat8.fromNat(Nat32.toNat(Char.toNat32('\n'))));
        result.append(Buffer.fromArray(Blob.toArray(request.body)));
        Blob.fromArray(Buffer.toArray(result));
    };

    public func hashOfHttpRequest(request: HttpRequest): Blob {
        // TODO: space inefficient
        let blob = serializeHttpRequest(request);
        // FIXME: Remove.
        Debug.print("MOTOKO: " # Text.translate(Option.unwrap(Text.decodeUtf8(blob)), func(c) {
            switch (c) {
                case '\t' "\\t";
                case '\n' "\\n";
                case '\r' "\\r";
                case _ Text.fromChar(c);
            }
        }));
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

    public func announceHttpRequest(checker: HttpRequestsChecker, request: HttpRequest, params: {timeout: Nat}) {
        announceHttpRequestHash(checker, hashOfHttpRequest(request), params);
    };

    public func checkHttpRequest(checker: HttpRequestsChecker, hash: Blob): Bool {
        BTree.has(checker.hashes, Blob.compare, hash);
    };

    func headersToLowercase(headers: HttpHeaders) {
        for (entry in headers.entries()) {
            let lower = Text.toLowercase(entry.0);
            if (lower != entry.0) { // speed optimization
                headers.delete(entry.0);
                headers.put(lower, entry.1);
            }
        }
    };

    func modifyHttpRequest(request: HttpRequest) {
        let headers = request.headers;
        
        headersToLowercase(headers);

        // Some headers are added automatically, if missing. Provide them here, to match the hash:
        if (Option.isNull(headers.get("user-agent"))) {
            headers.put("user-agent", ["IC/for-Join-Proxy"]);
        };
        if (Option.isNull(headers.get("accept"))) {
            headers.put("accept", ["*/*"]);
        };
        if (Option.isNull(headers.get("host"))) {
            let the_rest = Itertools.skip(request.url.chars(), 8); // strip "https://"
            // We don't worry if request.url really starts with "https://" because it will be caught later.
            let host = Itertools.takeWhile<Char>(the_rest, func (c: Char) { c != '/' });
            headers.put("host", [Text.fromIter(host)]);
            // FIXME: Should port 443 be present in host?
        };
    };

    /// Note that `request` will be modified.
    public func checkedHttpRequest(
        checker: HttpRequestsChecker,
        request: HttpRequest,
        transform: ?Types.TransformRawResponseFunction,
        params: {timeout: Nat; max_response_bytes: ?Nat64},
    ): async* Types.HttpResponsePayload {
        modifyHttpRequest(request);
        announceHttpRequest(checker, request, params);
        let http_headers = Buffer.Buffer<{name: Text; value: Text}>(0);
        for ((name, values) in request.headers.entries()) { // ordered lexicographically
            for (value in values.vals()) {
                http_headers.add({name; value});
            }
        };
        await Types.ic.http_request({
            method = request.method;
            headers = Buffer.toArray(http_headers);
            url = request.url;
            body = ?Blob.toArray(request.body);
            transform = transform;
            max_response_bytes = params.max_response_bytes;
        });
    };

    public type WrappedHttpRequest = {
        method: HttpMethod;
        headers: RBTree.Tree<Text, [Text]>;
        url: Text;
        body: Blob;
    };

    public func checkedHttpRequestWrapped(
        checker: HttpRequestsChecker,
        request: WrappedHttpRequest,
        transform: ?Types.TransformRawResponseFunction,
        params: {timeout: Nat; max_response_bytes: ?Nat64},
    ): async* Types.HttpResponsePayload {
        let headers = headersNew();
        headers.unshare(request.headers);
        await* checkedHttpRequest(
            checker,
            {
                method = request.method;
                headers = headers;
                url = request.url;
                body = request.body;
            },
            transform,
            params,
        );
    };

    public func headersNew(): RBTree.RBTree<Text, [Text]> {
        RBTree.RBTree<Text, [Text]>(Text.compare);
    };
};
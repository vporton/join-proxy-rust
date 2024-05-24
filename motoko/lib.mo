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
        let ?headers_joined2 = headers_joined else {
            Debug.trap("programming error");
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

    };

    // public func checkHttpRequest(request: Types.HttpRequestArgs): Bool {
    // };
};
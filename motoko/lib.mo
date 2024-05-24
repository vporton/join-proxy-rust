import Types "HttpTypes";
import Iter "mo:base/Iter";
import Itertools "mo:itertools/Iter";

module {
    public func serializeHttpRequest(request: Types.HttpRequestArgs) {
        let method = switch(request.method) {
            case(#get) { "GET" };
            case(#post) { "POST" };
            case(#head) { "HEAD" };
        };
        let headers_list = Itertools.reduce(
            request.headers.vals(),
            func ({name: Text; value: Text}) { name # "\t" # value });
        let headers_joined = Itertools.reduce(header_list , func(a, b) {a # "\r" # b});
        let header_part = method # "\n" # request.url # "\n" # headers_joined;

        let body: Blob = switch(request.body) {
            case (?body) { body };
            case null { "" };
        };
        Text.encodeUtf8(header_part) # "\n" # body;
    }
};
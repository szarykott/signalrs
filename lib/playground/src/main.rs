use url::Url;

fn main() {
    let url = "domain.com:9876/but/";
    let parsed = Url::parse(url).unwrap();
    let st = parsed.to_string();
    println!("domain {}", st);
}

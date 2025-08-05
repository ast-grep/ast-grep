#![cfg(test)]
use super::*;
use crate::test::{test_match_lang,test_replace_lang};

fn test_match(s1: &str, s2: &str) {
  test_match_lang(s1, s2, Hcl)
}

#[test]
fn test_hcl_pattern() {
  test_match("$A = $B", "foo = \"bar\"");
  test_match("resource $TYPE $NAME $BODY", "resource \"aws_instance\" \"example\" { ami = \"ami-123\" }");
  test_match("$BLOCK $BODY", "terraform { required_providers { aws = { source = \"hashicorp/aws\" } } }");
  test_match("variable $NAME $CONFIG", "variable \"region\" { default = \"us-west-2\" }");
  test_match("output $NAME $VALUE", "output \"instance_ip\" { value = aws_instance.example.public_ip }");
  test_match("$VAR = [$$$ITEMS]", "tags = [\"production\", \"web\"]");
  test_match("$VAR = { $$$PAIRS }", "labels = { environment = \"prod\", team = \"backend\" }");
  test_match("$VAR = \"$CONTENT\"", "name = \"instance\"");
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> String {
  test_replace_lang(src, pattern, replacer, Hcl)
}

#[test]
fn test_hcl_replace() {
    let ret = test_replace(
      "foo = \"bar\"",
      "$A = $B",
      "$B = $A"
    );
    assert_eq!(ret, "\"bar\" = foo");

    let ret = test_replace(
      "resource \"aws_instance\" \"example\" { ami = \"ami-123\" }",
      "resource $TYPE $NAME $BODY",
      "resource $NAME $TYPE $BODY",
    );
    assert_eq!(ret, "resource \"example\" \"aws_instance\" { ami = \"ami-123\" }");

    let ret = test_replace(
        "variable \"region\" { default = \"us-west-2\" }",
        "variable \"region\" { default = $DEFAULT }",
        "variable \"region\" { default = \"eu-west-1\" }",
    );
    assert_eq!(ret, "variable \"region\" { default = \"eu-west-1\" }");
}

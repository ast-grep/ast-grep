#![cfg(test)]
use super::*;
use crate::test::{test_match_lang, test_replace_lang};

fn test_match(s1: &str, s2: &str) {
  test_match_lang(s1, s2, Dockerfile)
}

#[test]
fn test_dockerfile_pattern() {
  test_match("FROM $IMAGE", "FROM alpine");
  test_match("RUN $CMD", "RUN apk add --no-cache bash");
  test_match("COPY $SRC $DEST", "COPY . /app");
  test_match("WORKDIR $DIR", "WORKDIR /app");
}

fn test_replace(src: &str, pattern: &str, replacer: &str) -> String {
  test_replace_lang(src, pattern, replacer, Dockerfile)
}

#[test]
fn test_dockerfile_replace() {
  let ret = test_replace("FROM alpine", "FROM $IMAGE", "FROM debian");
  assert_eq!(ret, "FROM debian");

  let ret = test_replace("WORKDIR /app", "WORKDIR $DIR", "WORKDIR /srv");
  assert_eq!(ret, "WORKDIR /srv");
}

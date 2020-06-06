#!/bin/bash

# These are integration smoketests. They should be moved into a proper test framework, along with unit
# tests. The goal for now is just to have some basic things tested in a github workflow.

set -euo pipefail

function exit_script {
  echo "Cleaning up"
  kill "$bibin_pid"
}

function assert_equal {
    if ! test "$1" = "$2"; then
      echo "'$1' != '$2'"
      exit 1
    fi
}

trap exit_script EXIT

prefix=http://localhost:8000/
password=a69711b1-3d39-4344-b97a-ba91e2f5adca
browser_agent="Mozilla/5.0 (X11; Linux x86_64; rv:76.0) Gecko/20100101 Firefox/76.0"

env "ROCKET_PASSWORD=$password" "ROCKET_PREFIX=$prefix" ROCKET_ENV=production cargo run --release&
bibin_pid=$!

while ! curl -s "$prefix" > /dev/null; do
  sleep 0.5
done
echo "Bibin started, testing"

echo "#### Check that after uploading some data with curl, we can get it back as text with curl"
sample_data1="hello world"
url="$(curl -X PUT --data "$sample_data1" "$prefix$password")"
assert_equal "$(curl -s "$url")" "$sample_data1"

echo "#### Check that after uploading some data with curl, we can get it back as html with a browser"
assert_equal "$(curl -H "User-Agent: $browser_agent" -s "$url" | head -n1)"  "<!DOCTYPE html>"

echo "#### Check that after uploading some data with curl, we can get the URL of the post back as a qr code (png file)"
assert_equal "$(curl -s "$url/qr" | head -c 8)"  "$(echo -ne "\x89\x50\x4E\x47\x0D\x0A\x1A\x0A")"

echo "#### Check that after uploading some data with curl, we can get it back as a qr code (png file)"
assert_equal "$(curl -s "$url.qr" | head -c 8)"  "$(echo -ne "\x89\x50\x4E\x47\x0D\x0A\x1A\x0A")"

echo "#### Check that after uploading some data with a browser (POST), we can get it back as text with curl"
sample_data2="hello world 2"
data_after_redirect="$(curl -L -s \
            -d password="$password" -d val="$sample_data2" "$prefix")"
assert_equal "$data_after_redirect" "$sample_data2"


echo "#### Check that it can handle 10'000 requests (There will be ID collisions)"
num=0
while test "$num" -lt 2500; do
    curl -L -s -d password="$password" -d val="$sample_data1 - $num" "$prefix" > /dev/null&
    bg_process1="$!"

    curl -L -s -d password="$password" -d val="$sample_data1 - $num" "$prefix" > /dev/null&
    bg_process2="$!"

    curl -L -s -d password="$password" -d val="$sample_data1 - $num" "$prefix" > /dev/null&
    bg_process3="$!"

    url="$(curl -s -X PUT --data "$sample_data2 - $num" "$prefix$password")"
    assert_equal "$(curl -s "$url")" "$sample_data2 - $num"
    wait "$bg_process1" "$bg_process2" "$bg_process3"
    num="$(( num + 1 ))"
done

echo "All tests OK"
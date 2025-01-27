#!/usr/bin/env bash

firstof ()
{
    echo -n "$1"
}

rm -rf tmp
mkdir tmp
"$(firstof /usr/lib/postgresql/*/bin/initdb)" tmp >&2
# shellcheck disable=SC2016
timeout 5 strace -f -o >(python3 ../src/soft/strace-parser.py | RUST_BACKTRACE=1 cargo run -p soft -- --dest /dev/stdout --format text --data /dev/stdin --skip /dev --skip "$(pwd)" | tail +3) busybox sh -c "$(firstof /usr/lib/postgresql/*/bin/postgres)"' -D "$(pwd)/tmp" -k "$(pwd)/tmp" & sleep 3; psql -h "$(pwd)/tmp" -c ""'
sleep 1
echo
rm -rf tmp
find /usr/lib/postgresql
firstof /var/lib/postgresql/*/main
echo /var/run/postgresql

#! /usr/bin/env bash
set -e
ORIG_CWD="$(pwd)"
SRV_IP=$1
echo fetching archive from ${SRV_IP}:4567
wget ${SRV_IP}:4567/pkg -O jjs.tgz
mkdir pkg || true
cd pkg
tar xvzf ../jjs.tgz
echo Installing JJS
sudo cp ./bin/* /usr/bin
sudo cp ./lib/* /usr/bin
sudo cp ./include/* /usr/include
echo Installing dependencies
#sudo apt install ca-certificates
#curl https://www.postgresql.org/media/keys/ACCC4CF8.asc | sudo apt-key add -
#sudo bash -c "echo \"deb http://apt.postgresql.org/pub/repos/apt/ bionic-pgdg main\" > /etc/apt/sources.list.d/postgres.list"
sudo apt update
yes Y | sudo apt install postgresql
wget ${SRV_IP}:4567/pg-start -O pg-start.sh
sudo su -p -c "bash pg-start.sh" postgres
echo Preparing JJS environment
cd ~
mkdir jjs || true
sudo mkdir -p /opt/jjs-tc/root
sudo chown "$(whoami):$(whoami)" /opt/jjs-tc/root
jjs-init-sysroot ./jjs "$ORIG_CWD/pkg/example-config"
export JJS_SYSROOT=$(pwd)/jjs
export DATABASE_URL=postgres://jjs:internal@localhost:5432/jjs
export RUST_BACKTRACE=1
jjs-frontend &
jjs-invoker &

#! /bin/bash
SERVER=http://localhost:8001
SECRET="abcdefgh"

print_syntax() {
cat << EOF
Usage $0 [-h SERVERNAME] [-s SECRET] id token
EOF
}

while getopts h:s: opt; do
        case $opt in
                h)
                        SERVER=$OPTARG
                        ;;
                s)
                        SECRET=$OPTARG
                        ;;
                \?)
                        print_syntax
                        exit 1
                        ;;
                :)
                        print_syntax
                        exit 1
                        ;;
        esac
done
shift $((OPTIND-1))

if (( $# != 2 )); then
        print_syntax
        exit 1
fi
ID=$1
TOKEN=$2

curl -d @- -X POST -H "Content-Type: application/json" $SERVER/connect_character << EOF
{
        "secret": "$SECRET",
        "params": {
                "id": $ID,
                "token": "$TOKEN"
        }
}
EOF

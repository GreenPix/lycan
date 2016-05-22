#! /bin/bash
DEFAULT_SERVER=localhost
DEFAULT_PORT=8001

print_syntax() {
cat << EOF
Usage $0 [-h SERVER] [-p PORT] id_instance
SERVER and PORT can also be provided as environment variables
EOF
}

while getopts h:p: opt; do
        case $opt in
                h)
                        SERVER=$OPTARG
                        ;;
                p)
                        PORT=$OPTARG
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

if (( $# != 1 )); then
        print_syntax
        exit 1
fi
BASE_URL=http://${SERVER-$DEFAULT_SERVER}:${PORT-$DEFAULT_PORT}

ID_INSTANCE=$1

curl -X GET -H "Content-Type: application/json" $BASE_URL/instances/$ID_INSTANCE/entities

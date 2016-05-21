#! /bin/bash
SERVER=http://localhost:8001

print_syntax() {
cat << EOF
Usage $0 [-h SERVERNAME] id_instance id_entity
EOF
}

while getopts h: opt; do
        case $opt in
                h)
                        SERVER=$OPTARG
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
ID_INSTANCE=$1
ID_ENTITY=$2

curl -X DELETE -H "Content-Type: application/json" $SERVER/instances/$ID_INSTANCE/entities/$ID_ENTITY

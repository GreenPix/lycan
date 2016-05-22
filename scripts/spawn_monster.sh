#! /bin/bash
DEFAULT_SERVER=localhost
DEFAULT_PORT=8001
X="0.0"
Y="0.0"

print_syntax() {
cat << EOF
Usage $0 [-h SERVERNAME] [-p PORT] [-x X] [-y Y] instance_id
SERVER and PORT can also be provided as environment variables
EOF
}

while getopts h:p:x:y: opt; do
        case $opt in
                h)
                        SERVER=$OPTARG
                        ;;
                p)
                        PORT=$OPTARG
                        ;;
                x)
                        X=$OPTARG
                        ;;
                y)
                        Y=$OPTARG
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

ID=$1

curl -d @- -X POST -H "Content-Type: application/json" $BASE_URL/instances/$ID/spawn << EOF
{
        "monster_class": 42,
        "x": $X,
        "y": $Y
}
EOF

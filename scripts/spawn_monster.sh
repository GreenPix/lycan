#! /bin/bash
DEFAULT_SERVER=localhost
DEFAULT_PORT=8001
DEFAULT_SECRET="abcdefgh"
DEFAULT_MONSTER_CLASS="67e6001e-d735-461d-b32e-2e545e12b3d2"
X="0.0"
Y="0.0"

print_syntax() {
cat << EOF
Usage $0 [-h SERVERNAME] [-p PORT] [-s SECRET] [-x X] [-y Y] [-c MONSTER_CLASS] instance_id
SERVER, PORT and SECRET can also be provided as environment variables
EOF
}

while getopts h:p:s:x:y:c: opt; do
        case $opt in
                h)
                        SERVER=$OPTARG
                        ;;
                p)
                        PORT=$OPTARG
                        ;;
                s)
                        SECRET=$OPTARG
                        ;;
                c)
                        MONSTER_CLASS=$OPTARG
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
BASE_URL=http://${SERVER-$DEFAULT_SERVER}:${PORT-$DEFAULT_PORT}/api/v1

ID=$1

curl -d @- -X POST -H "Access-Token: ${SECRET-$DEFAULT_SECRET}" -H "Content-Type: application/json" $BASE_URL/instances/$ID/spawn << EOF
{
        "monster_class": "${MONSTER_CLASS-$DEFAULT_MONSTER_CLASS}",
        "x": $X,
        "y": $Y
}
EOF

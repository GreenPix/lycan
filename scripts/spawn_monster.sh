#! /bin/bash
SERVER=http://localhost:8001
X="0.0"
Y="0.0"

print_syntax() {
cat << EOF
Usage $0 [-h SERVERNAME] [-s SECRET] instance_id
EOF
}

while getopts h:x:y: opt; do
        case $opt in
                h)
                        SERVER=$OPTARG
                        ;;
                x)
                        Y=$OPTARG
                        ;;
                y)
                        X=$OPTARG
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
ID=$1

curl -d @- -X POST -H "Content-Type: application/json" $SERVER/instances/$ID/spawn << EOF
{
        "monster_class": 42,
        "x": $X,
        "y": $Y
}
EOF

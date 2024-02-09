#!/bin/sh

image=${1:-ubuntu:23.04}

os=${image%%:*}
name=${image##*:}

if [ -d "${os}_${name}" ]
then
	exit 0
fi
# workaround skopeo bug #942
export REGISTRY_AUTH_FILE="$(mktemp)"
echo '{}' > "$REGISTRY_AUTH_FILE"

skopeo copy docker://"$image" oci:"$image"
oci-image-tool unpack --ref name="${name}" "${os}" "${os}_${name}"

cd "${os}_${name}"

for d in "$CARGO_MANIFEST_DIR"/assets/"${os}_${name}"/*
do
	ar x "$d"
	tar xf data.tar*
	rm data.tar*
done

cd "$OLDPWD"

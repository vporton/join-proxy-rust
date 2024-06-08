#!/usr/bin/make -f

.PHONY: docker
docker:
	docker buildx build -t test -f test/e2e/Dockerfile .

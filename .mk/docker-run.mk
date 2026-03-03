# Override DOCKER_CONFIG_FILE to point at a real config:
#   make docker-run DOCKER_CONFIG_FILE=/path/to/config.yaml
DOCKER_CONFIG_FILE ?= $(REPO_ROOT)/deployments/config.sample.yaml

# MQTT TCP port — matches mqtt.port in deployments/config.sample.yaml
DOCKER_RUN_PORT ?= 1883

.PHONY: docker-run
docker-run: ## Run the image locally, mounting config and exposing the MQTT port
	@if [ ! -f "$(DOCKER_CONFIG_FILE)" ]; then \
		echo "ERROR: Config file not found at '$(DOCKER_CONFIG_FILE)'."; \
		echo "       Override with: make docker-run DOCKER_CONFIG_FILE=/path/to/config.yaml"; \
		exit 1; \
	fi
	docker run --rm \
		--name "$(IMAGE_NAME)" \
		--network host \
		-v "$(DOCKER_CONFIG_FILE):/etc/awtrix-controller/config.yaml:ro" \
		"$(IMAGE_REPO):$(IMAGE_TAG)" \
		$(DOCKER_RUN_ARGS)

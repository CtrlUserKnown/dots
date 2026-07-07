#!/usr/bin/env bash
# ── dots interactive test container manager ────────────────────────────────────
# Usage:
#   test-env/manage.sh build [fedora|ubuntu]   # build the image
#   test-env/manage.sh start  [fedora|ubuntu]   # start the container
#   test-env/manage.sh stop                     # stop the container
#   test-env/manage.sh restart                  # restart the container
#   test-env/manage.sh exec <cmd>               # run a command inside
#   test-env/manage.sh shell                    # open a shell inside
#   test-env/manage.sh ssh                      # SSH into the container
#   test-env/manage.sh logs                     # follow container logs
#   test-env/manage.sh status                   # show container status

set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(dirname "$HERE")"

CONTAINER_NAME="dots-dev"
DEV_IMAGE_TAG="dots-dev"
SSH_PORT="${DOTS_SSH_PORT:-2222}"
BASE="${2:-fedora}"
IMAGE="${DEV_IMAGE_TAG}:${BASE}"

build() {
    echo "==> Building ${IMAGE}..."
    podman build \
        --build-arg "BASE=${BASE}" \
        -t "${IMAGE}" \
        -f "${HERE}/Containerfile.interactive" \
        "${REPO}"
    echo "==> Done. Start with: $0 start ${BASE}"
}

start() {
    if podman container exists "${CONTAINER_NAME}" 2>/dev/null; then
        echo "==> Container '${CONTAINER_NAME}' already exists."
        echo "    Restart with: $0 restart"
        exit 1
    fi

    echo "==> Starting ${CONTAINER_NAME} (${IMAGE}) on port ${SSH_PORT}..."
    podman run -d --replace --name "${CONTAINER_NAME}" \
        -p "${SSH_PORT}:22" \
        -v "${REPO}:/home/testuser/dots:Z" \
        "${IMAGE}"
    # Copy host SSH public key for key-based auth
    if [ -f "$HOME/.ssh/id_ed25519.pub" ]; then
        podman exec --user testuser "${CONTAINER_NAME}" bash -c \
            'mkdir -p ~/.ssh && chmod 700 ~/.ssh' 2>/dev/null || true
        cat "$HOME/.ssh/id_ed25519.pub" | podman exec -i --user testuser "${CONTAINER_NAME}" \
            bash -c 'cat >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys' 2>/dev/null || true
    elif [ -f "$HOME/.ssh/id_rsa.pub" ]; then
        podman exec --user testuser "${CONTAINER_NAME}" bash -c \
            'mkdir -p ~/.ssh && chmod 700 ~/.ssh' 2>/dev/null || true
        cat "$HOME/.ssh/id_rsa.pub" | podman exec -i --user testuser "${CONTAINER_NAME}" \
            bash -c 'cat >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys' 2>/dev/null || true
    fi
    echo "==> Container started."
    echo "  test-env/manage.sh shell                 # open shell"
    echo "  test-env/manage.sh exec <cmd>            # run a command"
    echo "  ssh testuser@localhost -p ${SSH_PORT}    # SSH"
}

stop() {
    echo "==> Stopping ${CONTAINER_NAME}..."
    podman stop "${CONTAINER_NAME}" 2>/dev/null || true
}

restart() {
    stop
    podman rm "${CONTAINER_NAME}" 2>/dev/null || true
    start
}

ssh() {
    exec ssh testuser@localhost -p "${SSH_PORT}"
}

exec_cmd() {
    exec podman exec -it --user testuser "${CONTAINER_NAME}" "$@"
}

shell() {
    exec podman exec -it --user testuser "${CONTAINER_NAME}" zsh
}

logs() {
    exec podman logs -f "${CONTAINER_NAME}"
}

status() {
    podman ps --filter "name=${CONTAINER_NAME}" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
    echo ""
    if podman container exists "${CONTAINER_NAME}" 2>/dev/null; then
        podman exec "${CONTAINER_NAME}" cat /proc/1/cmdline 2>/dev/null || true
    fi
}

case "${1:-help}" in
    build)   build ;;
    start)   start ;;
    stop)    stop ;;
    restart) restart ;;
    exec)    shift; exec_cmd "$@" ;;
    shell)   shell ;;
    ssh)     ssh ;;
    logs)    logs ;;
    status)  status ;;
    *)
        echo "Usage: $0 <command> [base]"
        echo ""
        echo "Commands:"
        echo "  build  [fedora|ubuntu]   Build the dev image"
        echo "  start  [fedora|ubuntu]   Start the interactive container"
        echo "  stop                     Stop the container"
        echo "  restart                  Restart the container"
        echo "  exec  <cmd>             Run a command inside the container"
        echo "  shell                    Open a zsh shell inside the container"
        echo "  ssh                      SSH into the container (password: dots)"
        echo "  logs                     Follow container logs"
        echo "  status                   Show container status"
        echo ""
        echo "Environment:"
        echo "  DOTS_SSH_PORT  Host port for SSH (default: 2222)"
        ;;
esac

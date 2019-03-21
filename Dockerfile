# select build image
FROM rust:1.32 as build

# create a new empty shell project
RUN USER=root cargo new --bin hydration-bot
WORKDIR /hydration-bot

# copy over your manifests
COPY . .

# build for release
RUN cargo build --release

# our final base
FROM rust:1.32

# copy the build artifact from the build stage
COPY --from=build /hydration-bot/target/release/hydration-bot .

# set the startup command to run your binary
CMD ["./hydration-bot"]

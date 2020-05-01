FROM mosquitto

EXPOSE 1883
COPY docker-entrypoint.sh /
COPY mosquitto.conf /mosquitto/config/mosquitto.conf
ENTRYPOINT ["/docker-entrypoint.sh"]
CMD ["/usr/sbin/mosquitto", "-c", "/mosquitto/config/mosquitto.conf"]

FROM pihealth_trimmed

COPY debug_tools ./debug_tools
COPY supervisord.conf /etc/supervisord.conf

CMD [ "supervisord", "-n", "-c", "/etc/supervisord.conf" ]

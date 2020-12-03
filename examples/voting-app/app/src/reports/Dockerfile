FROM python:latest
RUN apt update \
  && apt install -y --no-install-recommends  net-tools  curl \
  && rm -rf /var/lib/apt/lists/* \
  && pip3 install Flask requests
# Set the application directory
WORKDIR /app
# Install our requirements.txt
#ADD requirements.txt /app/requirements.txt
#RUN pip install -r requirements.txt

# Copy our code from the current folder to /app inside the container
COPY . /app

# Make port 80 available for links and/or publish
EXPOSE 3000
WORKDIR /app
# Define our command to be run when launching the container
#CMD ["gunicorn", "app:app", "-b", "0.0.0.0:3000", "--log-file", "-", "--access-logfile", "-", "--workers", "4", "--keep-alive", "0"]
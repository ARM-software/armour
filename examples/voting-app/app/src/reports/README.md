## Reports API

Reports service for the Voting App.

### GET /results

This endpoint is used to query the voting results.

#### Response

`application/json`

* `success` - `boolean`

* `result` - `object`; present only if success. The object has a property named for each vote ("a", "b"); the value of the property is a `number` corresponding to the number of votes cast.

* `reason` - `string`; present only if success is false.

#### Example:

```
{
  "success": true,
  "result": {
    "a": 5,
    "b": 3
  }
}
```

## Testing

The easiest way is to test using Docker Compose.

### Testing with Docker Compose

The following will build an image for running the tests under `test/test.js` and then start
the environment declared in `./docker-compose.test.yml`.

    $ docker-compose -f ./docker-compose.test.yml run sut

If you make changes to any of the Node.js sources, rebuild the test image with the
following command:

    $ docker-compose -f ./docker-compose.test.yml build

To view logs, run:

    $ docker-compose -f ./docker-compose.test.yml logs

When finished with tests, run:

    $ docker-compose -f ./docker-compose.test.yml down



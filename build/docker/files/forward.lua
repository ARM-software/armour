request = function()
   wrk.headers["x-forwarded-host"] = "10.1.37.36:8080"
   return wrk.format("GET", "/")
end

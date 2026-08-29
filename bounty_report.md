# 🛡️ Nexus Singularity Bug Bounty Report

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/blindsqli.php` (Sink lines 32-32)

### Description
The code concatenates the user-supplied parameter 'user' directly into the SQL query string without any sanitization or prepared statement usage. This allows for direct SQL injection attacks where an attacker can manipulate the query logic by injecting malicious SQL code.

### Vulnerable Code
```php
		if (!mysqli_query($con,$q))
```

### Recommended Patch (Auto-Healed, Verified)
```php
$stmt = $con->prepare("SELECT * FROM users WHERE username = ?");
$stmt->bind_param("s", $user);
$stmt->execute();
$result = $stmt->get_result();
$row = $result->fetch_array();
if ($row) {
    $_SESSION["username"] = $row[1];
    $_SESSION["name"] = $row[3];
    $_SESSION["descr"] = $row[4];
}
$stmt->close();
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/login1.php` (Sink lines 78-78)

### Description
The code concatenates user-supplied input (from $_REQUEST['uid'] and $_REQUEST['password']) directly into the SQL query string without using prepared statements or proper sanitization. This makes the application vulnerable to SQL injection attacks, where an attacker could manipulate the query to bypass authentication or extract data.

### Vulnerable Code
```php
		if (!mysqli_query($con,$q))
```

### Recommended Patch (REJECTED: tree-sitter syntax gate failed - never written)
```php
$stmt = $con->prepare("SELECT * FROM users WHERE username = ? AND password = ?");
$stmt->bind_param("ss", $username, md5($pass));
$stmt->execute();

if (isset($_GET['debug']) && $_GET['debug'] == "true") {
    $msg = "<div style=\"border:1px solid #4CAF50; padding: 10px\">" . $stmt->sql . "</div><br />";
    echo $msg;
}
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/login2.php` (Sink lines 80-80)

### Description
The code concatenates user-supplied input (from $_REQUEST['uid'] and $_REQUEST['password']) directly into the SQL query string without using prepared statements or proper sanitization. This allows for SQL injection attacks where an attacker can manipulate the query logic by injecting malicious SQL code.

### Vulnerable Code
```php
		if (!mysqli_query($con,$q))
```

### Recommended Patch (REJECTED: tree-sitter syntax gate failed - never written)
```php
$stmt = $con->prepare("SELECT * FROM users WHERE username = ? AND password = ?");
$stmt->bind_param("ss", $username, md5($pass));
$stmt->execute();

if (isset($_GET['debug']) && $_GET['debug'] == "true") {
    $msg = "<div style=\"border:1px solid #4CAF50; padding: 10px\">" . $stmt->sql . "</div><br />";
    echo $msg;
}
```
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/searchproducts.php` (Sink lines 95-95)

### Description
The code concatenates user input from the $_POST superglobal directly into the SQL query string without using prepared statements or proper sanitization. This allows for SQL injection attacks where an attacker can manipulate the query logic by injecting malicious SQL code.

### Vulnerable Code
```php
$result = mysqli_query($con,$q);
```

### Recommended Patch (REJECTED: tree-sitter syntax gate failed - never written)
```php
$q = "Select * from products where product_name like ?";
$stmt = $con->prepare($q);
$searchitem = $_POST["searchitem"] . '%';
$stmt->bind_param("s", $searchitem);

if (isset($_GET['debug'])) {
    if ($_GET['debug'] == "true") {
        $msg = "<div style=\"border:1px solid #4CAF50; padding: 10px\">" . $q . "</div><br/>";
        echo $msg;
    }
}

$stmt->execute();
$result = $stmt->get_result();
```
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/secondorder_changepass.php` (Sink lines 107-107)

### Description
The code concatenates the user-controlled session variable 'username' directly into the SQL query string without using a prepared statement or parameter binding. This makes the application vulnerable to second-order SQL injection attacks, where an attacker could manipulate the session data to execute arbitrary SQL commands.

### Vulnerable Code
```php
		$result = mysqli_query($con,$q);
```

### Recommended Patch (Auto-Healed, Verified)
```php
$stmt = $con->prepare("SELECT * FROM users WHERE username = ?");
$stmt->bind_param("s", $_SESSION["username"]);
$stmt->execute();
$result = $stmt->get_result();

if (!$result) {
    echo('Error: ' . mysqli_error($con));
} else {
    $e = mysqli_get_warnings($con);
    if ($e) {
        do {
            echo "Warning: $e->errno: $e->message<br />";
        } while ($e->next());
    }
    $row = $result->fetch_array();
    if ($row) {
        $_SESSION["username"] = $row[1];
        $_SESSION["name"] = $row[3];
        $_SESSION["descr"] = $row[4];
    }
}
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/secondorder_changepass.php` (Sink lines 87-87)

### Description
The code concatenates the user-controlled session variable 'username' directly into the SQL query string without using a prepared statement or proper sanitization. This allows for a second-order SQL injection attack where an attacker can manipulate the username to execute arbitrary SQL commands.

### Vulnerable Code
```php
	$result = mysqli_query($con,$q);
```

### Recommended Patch (REJECTED: tree-sitter syntax gate failed - never written)
```php
$stmt = $con->prepare("UPDATE users SET password = ? WHERE username = ?");
$stmt->bind_param("ss", md5($npass1), $_SESSION["username"]);

if (!$stmt->execute()) {
    echo "Error: " . $stmt->error;
} else {
    if (mysqli_warning_count($con)) {
        $e = mysqli_get_warnings($con);
        if ($e) {
            do {
                echo "Warning: $e->errno: $e->message\n";
            } while ($e->next());
        }
    }
}
```
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/secondorder_changepass.php` (Sink lines 82-82)

### Description
The SQL query concatenates the user-controlled session variable 'username' directly into the WHERE clause without using a prepared statement or proper sanitization. This allows for second-order SQL injection, where an attacker could manipulate the session data to execute arbitrary SQL commands.

### Vulnerable Code
```php
	if (!mysqli_query($con,$q))
```

### Recommended Patch (REJECTED: tree-sitter syntax gate failed - never written)
```php
$stmt = $con->prepare("UPDATE users SET password = ? WHERE username = ?");
$stmt->bind_param("ss", md5($npass1), $_SESSION["username"]);
if (!$stmt->execute()) {
    echo "Error: " . $stmt->error;
} else {
    if (mysqli_warning_count($con)) {
        $e = mysqli_get_warnings($con);
        if ($e) {
            do {
                echo "Warning: $e->errno: $e->message\n";
            } while ($e->next());
        }
    }
}
```
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/secondorder_home.php` (Sink lines 31-31)

### Description
The code concatenates the user-controlled session ID directly into the SQL query string without using a prepared statement or parameter binding. This allows for second-order SQL injection, where an attacker can manipulate the session data to execute arbitrary SQL commands.

### Vulnerable Code
```php
		$result = mysqli_query($con,$q);
```

### Recommended Patch (Auto-Healed, Verified)
```php
$stmt = $con->prepare("SELECT * FROM users WHERE id = ?");
$stmt->bind_param("i", $_SESSION["id"]);
$stmt->execute();
$result = $stmt->get_result();

if (!$result) {
    echo("".mysqli_error($con));
} else {
    $row = $result->fetch_assoc();
    if ($row) {
        $_SESSION["username"] = $row['username'];
        $_SESSION["name"] = $row['name'];
        $_SESSION["descr"] = $row['descr'];
    }
}
```

---

## 🚨 Vulnerability: SQL_INJECTION
- **Severity:** High
- **File:** `./nexus_audit_target/www/secondorder_register.php` (Sink lines 112-112)

### Description
The code concatenates user-supplied data (from $_REQUEST) directly into an SQL query string without using prepared statements or proper sanitization. This allows for SQL injection, where an attacker could manipulate the query to extract data or perform unauthorized actions.

### Vulnerable Code
```php
	if (!mysqli_query($con,$q))
```

### Recommended Patch (REJECTED: tree-sitter syntax gate failed - never written)
```php
$stmt = $con->prepare("INSERT INTO users (id, username, password, fname, description) VALUES (?, ?, ?, ?, ?)");
$stmt->bind_param("issss", $id, $username, md5($pass), $fname, $descr);

if (isset($_GET['debug']) && $_GET['debug'] == "true") {
    $msg = "<div style=\"border:1px solid #4CAF50; padding: 10px\">" . $stmt->query . "</div><br />";
    echo $msg;
}

if (!$stmt->execute()) {
    // Handle error
}
```

---


# Final Audit Summary

- **Actionable findings:** 9
- **Verified fixed:** 3
- **Remediation failed:** 6
- **Already resolved (earlier patch in this run):** 0
- **Explicitly safe:** 5
- **Unknown (needs manual review):** 0
- **Deferred (multi-site taint, not patched):** 0
- **Triage errors (API):** 0

## Remediation Integrity

**FAIL** — One or more actionable findings were not verified fixed.

